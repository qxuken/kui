//! NativeActivity's content rect can stay at the full surface size on
//! edge-to-edge windows. Read the decor view's applied IME insets on its UI thread.
use super::{AndroidApp, Edges, Window};
use jni::{jni_sig, jni_str, objects::JValue};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use winit::platform::android::WindowExtAndroid;

#[derive(Default, Copy, Clone, PartialEq)]
struct WindowInsets {
    safe: Edges,
    keyboard: i32,
}

#[derive(Default)]
pub(super) struct AndroidInsets {
    insets: Arc<Mutex<WindowInsets>>,
    next_check: Option<Instant>,
    dirty: bool,
    pending: Arc<AtomicBool>,
}

impl AndroidInsets {
    pub(super) fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub(super) fn layout(&self, window: &Window, width: f32, height: f32) -> (f32, Edges) {
        let scale = window.scale_factor() as f32;
        let WindowInsets { safe, keyboard } = *self.insets.lock().unwrap();
        let rect = window.content_rect();
        let mut edges = Edges {
            l: safe.l / scale,
            r: safe.r / scale,
            t: safe.t / scale,
            b: safe.b / scale,
        };
        if rect.right > rect.left && rect.bottom > rect.top {
            edges.l = edges.l.max(rect.left as f32 / scale);
            edges.r = edges.r.max(width - rect.right as f32 / scale);
            edges.t = edges.t.max(rect.top as f32 / scale);
            edges.b = edges.b.max(height - rect.bottom as f32 / scale);
        }
        let keyboard = keyboard as f32 / scale;
        edges.b = (edges.b - keyboard).max(0.);
        ((height - keyboard).max(0.), edges)
    }

    pub(super) fn check(
        &mut self,
        app: &AndroidApp,
        window: &Arc<Window>,
        editing: bool,
    ) -> Option<Instant> {
        // winit doesn't expose an IME-insets event. While an editor has focus
        // (or its keyboard is closing), sample without forcing idle GPU frames.
        let now = Instant::now();
        let current = *self.insets.lock().unwrap();
        if !self.dirty && !editing && current.keyboard == 0 {
            self.next_check = None;
            return None;
        }
        if std::mem::take(&mut self.dirty) || self.next_check.is_none_or(|next| now >= next) {
            self.next_check = Some(now + Duration::from_millis(100));
            if self.pending.swap(true, Ordering::AcqRel) {
                self.dirty = true;
                return self.next_check;
            }
            let pending = self.pending.clone();
            let app_ref = app.clone();
            let shared = self.insets.clone();
            let window = window.clone();
            app.run_on_java_main_thread(Box::new(move || {
                let sampled = window_insets(&app_ref);
                pending.store(false, Ordering::Release);
                if let Ok(insets) = sampled {
                    let mut previous = shared.lock().unwrap();
                    if *previous != insets {
                        *previous = insets;
                        window.request_redraw();
                    }
                }
            }));
        }
        self.next_check
    }
}

fn window_insets(app: &AndroidApp) -> jni::errors::Result<WindowInsets> {
    super::android_editor::with_activity(app, |env, activity| {
        let window = env
            .call_method(
                activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;
        let view = env
            .call_method(
                &window,
                jni_str!("getDecorView"),
                jni_sig!("()Landroid/view/View;"),
                &[],
            )?
            .l()?;
        let insets = env
            .call_method(
                &view,
                jni_str!("getRootWindowInsets"),
                jni_sig!("()Landroid/view/WindowInsets;"),
                &[],
            )?
            .l()?;
        if insets.is_null() {
            return Ok(WindowInsets::default());
        }
        if AndroidApp::sdk_version() >= 30 {
            let ime = env
                .call_static_method(
                    jni_str!("android/view/WindowInsets$Type"),
                    jni_str!("ime"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            let area = env
                .call_method(
                    &insets,
                    jni_str!("getInsets"),
                    jni_sig!("(I)Landroid/graphics/Insets;"),
                    &[JValue::Int(ime)],
                )?
                .l()?;
            let keyboard = env
                .get_field(&area, jni_str!("bottom"), jni_sig!("I"))?
                .i()?;
            let mut safe_types = 0;
            for name in [jni_str!("systemBars"), jni_str!("displayCutout")] {
                safe_types |= env
                    .call_static_method(
                        jni_str!("android/view/WindowInsets$Type"),
                        name,
                        jni_sig!("()I"),
                        &[],
                    )?
                    .i()?;
            }
            let safe = env
                .call_method(
                    &insets,
                    jni_str!("getInsets"),
                    jni_sig!("(I)Landroid/graphics/Insets;"),
                    &[JValue::Int(safe_types)],
                )?
                .l()?;
            let mut edge = |name| -> jni::errors::Result<f32> {
                Ok(env.get_field(&safe, name, jni_sig!("I"))?.i()? as f32)
            };
            return Ok(WindowInsets {
                keyboard,
                safe: Edges {
                    l: edge(jni_str!("left"))?,
                    r: edge(jni_str!("right"))?,
                    t: edge(jni_str!("top"))?,
                    b: edge(jni_str!("bottom"))?,
                },
            });
        }
        let mut inset = |name| env.call_method(&insets, name, jni_sig!("()I"), &[])?.i();
        let bottom = inset(jni_str!("getSystemWindowInsetBottom"))?;
        let bars = inset(jni_str!("getStableInsetBottom"))?;
        let mut safe = Edges {
            l: inset(jni_str!("getSystemWindowInsetLeft"))? as f32,
            r: inset(jni_str!("getSystemWindowInsetRight"))? as f32,
            t: inset(jni_str!("getSystemWindowInsetTop"))? as f32,
            b: bottom.min(bars) as f32,
        };
        if AndroidApp::sdk_version() >= 28 {
            let cutout = env
                .call_method(
                    &insets,
                    jni_str!("getDisplayCutout"),
                    jni_sig!("()Landroid/view/DisplayCutout;"),
                    &[],
                )?
                .l()?;
            if !cutout.is_null() {
                let mut edge = |name| -> jni::errors::Result<f32> {
                    Ok(env.call_method(&cutout, name, jni_sig!("()I"), &[])?.i()? as f32)
                };
                safe.l = safe.l.max(edge(jni_str!("getSafeInsetLeft"))?);
                safe.r = safe.r.max(edge(jni_str!("getSafeInsetRight"))?);
                safe.t = safe.t.max(edge(jni_str!("getSafeInsetTop"))?);
                safe.b = safe.b.max(edge(jni_str!("getSafeInsetBottom"))?);
            }
        }
        Ok(WindowInsets {
            keyboard: if bottom > bars { bottom } else { 0 },
            safe,
        })
    })
}
