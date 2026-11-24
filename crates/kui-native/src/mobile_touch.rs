//! Translate one finger into existing core inputs. Defer a press until lift
//! or until movement distinguishes a drag from panning a scroll region.
use kui_core::touch::{Glide, PanSamples, TOUCH_SLOP};
use kui_core::{InputEvent, Vec2, input::Interaction};
use std::collections::HashSet;
use winit::event::TouchPhase;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GestureAction {
    Pan,
    Drag,
    TapOnly,
}

impl GestureAction {
    pub fn at(input: &Interaction, point: Vec2) -> Self {
        if input.prefers_drag_at(point) {
            Self::Drag
        } else if input.prefers_scroll_at(point) {
            Self::Pan
        } else {
            Self::TapOnly
        }
    }
}

struct Gesture {
    start: Vec2,
    last: Vec2,
    action: GestureAction,
    moved: bool,
    claimed: bool,
    samples: PanSamples,
}

impl Gesture {
    fn motion(&mut self, at: Vec2, time: f64, send: &mut impl FnMut(InputEvent) -> bool) {
        let delta = at.minus(self.last);
        let first = !self.moved;
        if delta == Vec2::ZERO || first && delta.x.hypot(delta.y) <= TOUCH_SLOP {
            return;
        }
        match self.action {
            GestureAction::Pan => {
                if first {
                    send(InputEvent::CursorMoved(self.start));
                }
                self.claimed = send(InputEvent::ScrollGesture {
                    delta,
                    begins: first,
                });
                self.samples.record(time, at);
            }
            GestureAction::Drag => {
                if first {
                    send(InputEvent::CursorMoved(self.start));
                    send(InputEvent::mouse_down(1));
                }
                send(InputEvent::CursorMoved(at));
            }
            GestureAction::TapOnly => {}
        }
        self.moved = true;
        self.last = at;
    }

    fn glide(&self, time: f64) -> Option<Glide> {
        if !self.claimed {
            return None;
        }
        self.samples.release(time, 1.)
    }
}

#[derive(Default)]
pub(crate) struct Contacts {
    down: HashSet<u64>,
    gesture: Option<Gesture>,
    glide: Option<Glide>,
}

impl Contacts {
    pub(super) fn handle(
        &mut self,
        phase: TouchPhase,
        id: u64,
        at: Vec2,
        action: GestureAction,
        time: f64,
        // The flag marks the press of a confirmed tap.
        mut send: impl FnMut(InputEvent, bool) -> bool,
    ) -> bool {
        let mut tapped = false;
        match phase {
            TouchPhase::Started => {
                self.glide = None;
                if !self.down.insert(id) {
                    return false;
                }
                if self.down.len() == 1 {
                    let mut samples = PanSamples::default();
                    samples.record(time, at);
                    self.gesture = Some(Gesture {
                        start: at,
                        last: at,
                        moved: false,
                        action,
                        claimed: false,
                        samples,
                    });
                } else {
                    self.cancel(&mut |input| send(input, false));
                }
            }
            TouchPhase::Moved if self.down.contains(&id) => {
                if let Some(g) = &mut self.gesture {
                    g.motion(at, time, &mut |input| send(input, false));
                }
            }
            TouchPhase::Ended if self.down.remove(&id) => {
                if let Some(mut g) = self.gesture.take() {
                    g.motion(at, time, &mut |input| send(input, false));
                    self.glide = g.glide(time);
                    if !g.moved {
                        tapped = true;
                        send(InputEvent::CursorMoved(g.start), false);
                        send(InputEvent::mouse_down(1), true);
                    }
                    if !g.moved || g.action == GestureAction::Drag {
                        send(InputEvent::CursorMoved(at), false);
                        send(InputEvent::mouse_up(), false);
                    }
                    send(InputEvent::CursorLeft, false);
                }
            }
            TouchPhase::Cancelled if self.down.remove(&id) => {
                self.cancel(&mut |input| send(input, false))
            }
            _ => {}
        }
        tapped
    }

    fn cancel(&mut self, send: &mut impl FnMut(InputEvent) -> bool) {
        self.stop_glide();
        send(InputEvent::CursorLeft);
        if self
            .gesture
            .take()
            .is_some_and(|g| g.moved && g.action == GestureAction::Drag)
        {
            send(InputEvent::mouse_up());
        }
    }

    /// Apply the exact integral of exponentially decaying velocity. A new
    /// touch stops the glide before the next frame. A stalled frame cannot
    /// accumulate a jump when the app returns from the background.
    pub fn advance(&mut self, time: f64, mut send: impl FnMut(InputEvent) -> bool) -> bool {
        let Some(glide) = &mut self.glide else {
            return false;
        };
        if glide.advance(time, |delta| {
            send(InputEvent::ScrollGesture {
                delta,
                begins: false,
            })
        }) {
            return true;
        }
        self.glide = None;
        false
    }

    pub fn stop_glide(&mut self) {
        self.glide = None;
    }

    pub fn reset(&mut self, mut send: impl FnMut(InputEvent) -> bool) {
        self.down.clear();
        self.cancel(&mut send);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::{Core, NodeSpec, Size};

    fn flick(release: f64, claimed: bool) -> Contacts {
        let mut contacts = Contacts::default();
        for (phase, at, time) in [
            (TouchPhase::Started, 100., 0.),
            (TouchPhase::Moved, 80., 0.02),
            (TouchPhase::Moved, 60., 0.04),
            (TouchPhase::Ended, 60., release),
        ] {
            contacts.handle(
                phase,
                1,
                Vec2::new(10., at),
                GestureAction::Pan,
                time,
                |_, _| claimed,
            );
        }
        contacts
    }

    #[test]
    fn flick_continues_the_latched_scroll_at_any_refresh_rate() {
        let travel = |frames: u32| {
            let mut contacts = flick(0.04, true);
            let mut y = 0.;
            for i in 1..=frames {
                assert!(
                    contacts.advance(0.04 + 0.2 * f64::from(i) / f64::from(frames), |event| {
                        let InputEvent::ScrollGesture {
                            delta,
                            begins: false,
                        } = event
                        else {
                            panic!("momentum must continue the original scroll");
                        };
                        y += delta.y;
                        true
                    })
                );
            }
            y
        };
        let expected = -1000. * (1. - (-1f32).exp()) / 5.;
        assert!((travel(12) - expected).abs() < 0.01);
        assert!((travel(24) - expected).abs() < 0.01);
    }

    #[test]
    fn sparse_delivery_keeps_the_flick_before_a_small_lift_correction() {
        // Android emulator trace, in logical pixels and callback seconds.
        let mut contacts = Contacts::default();
        for (phase, y, time) in [
            (TouchPhase::Started, 116.73, 0.0),
            (TouchPhase::Moved, 62.13, 0.158),
            (TouchPhase::Ended, 60.73, 0.315),
        ] {
            contacts.handle(
                phase,
                1,
                Vec2::new(50., y),
                GestureAction::Pan,
                time,
                |_, _| true,
            );
        }
        let mut travelled = 0.;
        contacts.advance(0.4, |event| {
            if let InputEvent::ScrollGesture {
                delta,
                begins: false,
            } = event
            {
                travelled -= delta.y;
            }
            true
        });
        assert!(travelled > 5.6, "missing forward glide: {travelled}");
    }

    #[test]
    fn a_sparse_reversal_uses_the_new_direction_and_a_hold_drops_the_old_stroke() {
        let release = |y, time| {
            let mut contacts = Contacts::default();
            for (phase, y, time) in [
                (TouchPhase::Started, 116., 0.),
                (TouchPhase::Moved, 62., 0.158),
                (TouchPhase::Ended, y, time),
            ] {
                contacts.handle(
                    phase,
                    1,
                    Vec2::new(50., y),
                    GestureAction::Pan,
                    time,
                    |_, _| true,
                );
            }
            let mut dy = None;
            contacts.advance(time + 0.016, |event| {
                if let InputEvent::ScrollGesture { delta, .. } = event {
                    dy = Some(delta.y);
                }
                true
            });
            dy
        };
        assert!(release(80., 0.315).unwrap() > 0.);
        assert!(release(61., 0.6).is_none());
    }

    #[test]
    fn a_batched_final_move_keeps_a_velocity_baseline() {
        let mut contacts = Contacts::default();
        for (phase, y, time) in [
            (TouchPhase::Started, 100., 0.),
            (TouchPhase::Moved, 80., 0.02),
            (TouchPhase::Ended, 30., 0.12),
        ] {
            contacts.handle(
                phase,
                1,
                Vec2::new(5., y),
                GestureAction::Pan,
                time,
                |_, _| true,
            );
        }
        assert!(contacts.advance(0.136, |_| true));
        assert!(
            !contacts.advance(0.15, |_| false),
            "stop when the scroll target is gone"
        );
    }

    #[test]
    fn holding_still_an_unclaimed_pan_and_a_long_frame_do_not_glide() {
        for (mut contacts, next) in [
            (flick(0.2, true), 0.216),
            (flick(0.04, false), 0.056),
            (flick(0.04, true), 1.),
        ] {
            assert!(!contacts.advance(next, |_| panic!("unexpected scroll")));
        }
    }

    #[test]
    fn a_stationary_lift_at_the_next_sparse_sample_keeps_the_flick() {
        let mut contacts = Contacts::default();
        for (phase, y, time) in [
            (TouchPhase::Started, 120., 0.),
            (TouchPhase::Moved, 100., 0.10),
            (TouchPhase::Moved, 70., 0.23),
            (TouchPhase::Ended, 70., 0.38),
        ] {
            contacts.handle(
                phase,
                1,
                Vec2::new(0., y),
                GestureAction::Pan,
                time,
                |_, _| true,
            );
        }
        assert!(contacts.advance(0.40, |_| true));
    }

    #[test]
    fn a_slow_foreground_frame_still_presents_the_release() {
        let mut contacts = Contacts::default();
        for (phase, y, time) in [
            (TouchPhase::Started, 120., 0.),
            (TouchPhase::Moved, 100., 0.04),
            (TouchPhase::Ended, 60., 0.36),
        ] {
            contacts.handle(
                phase,
                1,
                Vec2::new(0., y),
                GestureAction::Pan,
                time,
                |_, _| true,
            );
        }
        let mut travel = 0.;
        contacts.advance(0.64, |event| {
            if let InputEvent::ScrollGesture { delta, .. } = event {
                travel -= delta.y;
            }
            true
        });
        assert!((10. ..25.).contains(&travel), "release travel: {travel}");
    }

    #[test]
    fn new_contact_stops_gliding_and_a_second_finger_cancels_the_pending_click() {
        let mut contacts = flick(0.04, true);
        let at = Vec2::new(20., 20.);
        let mut events = Vec::new();
        for (phase, id) in [
            (TouchPhase::Started, 1),
            (TouchPhase::Started, 2),
            (TouchPhase::Ended, 1),
            (TouchPhase::Ended, 2),
        ] {
            contacts.handle(phase, id, at, GestureAction::Pan, 0.05, |e, _| {
                events.push(e);
                true
            });
            assert!(!contacts.advance(0.066, |_| panic!("new touch must stop momentum")));
        }
        assert_eq!(events, [InputEvent::CursorLeft]);
    }

    #[test]
    fn a_drag_owner_gets_pointer_motion_and_no_momentum() {
        let mut contacts = Contacts::default();
        let start = Vec2::new(20., 20.);
        let end = Vec2::new(50., 20.);
        let mut events = Vec::new();
        for (phase, at) in [
            (TouchPhase::Started, start),
            (TouchPhase::Moved, end),
            (TouchPhase::Ended, end),
        ] {
            contacts.handle(phase, 1, at, GestureAction::Drag, 0.02, |e, _| {
                events.push(e);
                true
            });
        }
        assert_eq!(
            events,
            [
                InputEvent::CursorMoved(start),
                InputEvent::mouse_down(1),
                InputEvent::CursorMoved(end),
                InputEvent::CursorMoved(end),
                InputEvent::mouse_up(),
                InputEvent::CursorLeft
            ]
        );
        assert!(!contacts.advance(0.036, |_| panic!("drag produced momentum")));
    }

    #[test]
    fn interruption_discards_a_pending_tap() {
        let mut contacts = Contacts::default();
        let at = Vec2::new(10., 10.);
        let mut events = Vec::new();
        contacts.handle(
            TouchPhase::Started,
            1,
            at,
            GestureAction::Pan,
            0.,
            |e, _| {
                events.push(e);
                true
            },
        );
        contacts.reset(|e| {
            events.push(e);
            true
        });
        contacts.handle(TouchPhase::Ended, 1, at, GestureAction::Pan, 0., |e, _| {
            events.push(e);
            true
        });
        assert_eq!(events, [InputEvent::CursorLeft]);
    }

    #[test]
    fn swipe_across_standalone_button_does_not_click() {
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(200., 150.), 1.);
        ui.leaf(NodeSpec::column().size(160., 80.).on_click("increment"));
        ui.finish();

        let start = Vec2::new(30., 70.);
        let end = Vec2::new(30., 10.);
        let action = GestureAction::at(&core.interaction, start);
        assert!(action == GestureAction::TapOnly);
        let mut contacts = Contacts::default();
        let mut clicks = 0;
        let mut send = |input, _| {
            clicks += core
                .handle_input(input)
                .iter()
                .filter(|e| e.event.name() == "click")
                .count();
            true
        };

        contacts.handle(TouchPhase::Started, 1, start, action, 0., &mut send);
        contacts.handle(TouchPhase::Moved, 1, end, action, 0.03, &mut send);
        contacts.handle(TouchPhase::Ended, 1, end, action, 0.04, &mut send);
        assert_eq!(clicks, 0);
    }

    #[test]
    fn swipe_inside_button_scrolls_without_clicking_and_a_tap_still_clicks() {
        let mut core = Core::new();
        let frame = |core: &mut Core| {
            let mut ui = core.frame(Size::new(200., 150.), 1.);
            let list = ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
                ui.leaf(NodeSpec::column().size(160., 80.).on_click("increment"));
                ui.leaf(NodeSpec::column().size(160., 400.));
            });
            ui.finish();
            list
        };
        let list = frame(&mut core);
        let mut contacts = Contacts::default();
        let mut finger = |core: &mut Core, phase, y| {
            let at = Vec2::new(30., y);
            let action = GestureAction::at(&core.interaction, at);
            let mut clicks = 0;
            contacts.handle(phase, 1, at, action, 0., |input, _| {
                clicks += core
                    .handle_input(input)
                    .iter()
                    .filter(|e| e.event.name() == "click")
                    .count();
                true
            });
            clicks
        };
        assert_eq!(finger(&mut core, TouchPhase::Started, 50.), 0);
        assert_eq!(finger(&mut core, TouchPhase::Moved, 30.), 0);
        assert_eq!(finger(&mut core, TouchPhase::Ended, 30.), 0);
        frame(&mut core);
        assert_eq!(core.scroll_offset(list).y, 20.);
        // A tap with a little jitter activates the same, now scrolled, button.
        assert_eq!(finger(&mut core, TouchPhase::Started, 26.), 0);
        assert_eq!(finger(&mut core, TouchPhase::Ended, 28.), 1);
    }
}
