use crate::host::{RUNNER, browser_text, publish_browser_bytes};
use kui_core::audio::{AudioCommand, PlaybackId};
use kui_core::captures::CaptureCommand;
use kui_core::event::{CapturePhase, Captured};
use kui_core::{InputEvent, KeyCode, KeyMods, KeyPress, MenuAction, Value, Vec2};
use serde_json::{Value as Json, json};
use std::cell::RefCell;

thread_local! {
    static CAPTURES: RefCell<Vec<Option<Value>>> = const { RefCell::new(Vec::new()) };
}

fn capture(kind: &str, path: String, message: Option<Value>) -> Json {
    let id = CAPTURES.with_borrow_mut(|requests| {
        requests.push(message);
        requests.len() - 1
    });
    json!({ "kind": kind, "path": path, "id": id })
}

#[unsafe(no_mangle)]
pub extern "C" fn take_demo_effects() -> usize {
    let requests = RUNNER.with_borrow_mut(|runner| {
        let core = runner.core_mut();
        let mut requests = Vec::new();
        for action in core.take_menu_actions() {
            match action {
                MenuAction::SetClipboard { text, .. } => {
                    requests.push(json!({ "kind": "copy", "text": text }));
                }
                MenuAction::Paste => requests.push(json!({ "kind": "paste" })),
                _ => {}
            }
        }
        for dialog in core.take_file_requests() {
            let extensions = dialog
                .filters
                .iter()
                .flat_map(|filter| &filter.extensions)
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>();
            requests.push(json!({
                "kind": "files", "mode": dialog.mode.name(), "multiple": dialog.multiple,
                "accept": extensions.join(",")
            }));
        }
        for command in core.take_capture_commands() {
            match command {
                CaptureCommand::Still { path, message } => {
                    requests.push(capture("still", path, message));
                }
                CaptureCommand::Clipboard { message } => {
                    requests.push(capture("copy-image", String::new(), message));
                }
                CaptureCommand::Record { path, message } => {
                    requests.push(capture("record", path, message));
                }
                CaptureCommand::Stop => requests.push(json!({ "kind": "stop-recording" })),
                CaptureCommand::Devtools(_) => {}
            }
        }
        for command in core.take_audio_commands() {
            requests.push(match command {
                AudioCommand::Play {
                    playback,
                    sound,
                    volume,
                    looped,
                    ..
                } => {
                    let bytes = core.resources.sound(sound).map(|bytes| bytes.to_vec());
                    json!({
                        "kind": "play", "id": playback.0, "bytes": bytes,
                        "sound": sound.to_ffi().to_string(), "volume": volume, "loop": looped,
                    })
                }
                AudioCommand::Stop { playback, .. } => json!({ "kind": "stop", "id": playback.0 }),
                AudioCommand::SetVolume {
                    playback, volume, ..
                } => {
                    json!({ "kind": "volume", "id": playback.0, "volume": volume })
                }
                AudioCommand::Pause { playback, .. } => {
                    json!({ "kind": "pause", "id": playback.0 })
                }
                AudioCommand::Resume { playback, .. } => {
                    json!({ "kind": "resume", "id": playback.0 })
                }
                AudioCommand::MasterVolume { volume, .. } => {
                    json!({ "kind": "master", "volume": volume })
                }
                AudioCommand::Unload { sound } => {
                    json!({ "kind": "unload", "sound": sound.to_ffi().to_string() })
                }
            });
        }
        requests
    });
    if requests.is_empty() {
        return 0;
    }
    publish_browser_bytes(serde_json::to_vec(&requests).expect("serializable demo effects"))
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_capture_result(
    id: usize,
    ok: u32,
    width: u32,
    height: u32,
    frames: u32,
    seconds: f64,
) {
    let message = CAPTURES.with_borrow_mut(|requests| requests.get_mut(id).and_then(Option::take));
    let (phase, path, error) = if ok != 0 {
        (CapturePhase::Saved, browser_text(), None)
    } else {
        (CapturePhase::Failed, String::new(), Some(browser_text()))
    };
    RUNNER.with_borrow_mut(|runner| {
        runner.core_mut().captured(
            message,
            Captured {
                phase,
                path,
                width,
                height,
                frames,
                seconds,
                error,
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_audio_ended(id: u32, refused: u32) {
    RUNNER.with_borrow_mut(|runner| {
        let core = runner.core_mut();
        let playback = PlaybackId(u64::from(id));
        if refused == 0 {
            core.audio_ended(playback);
        } else {
            core.audio_refused(playback);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_key(modifiers: u32, down: u32, time: f64) -> u32 {
    let input = browser_text();
    let (layout, physical) = input.split_once('\0').unwrap_or((&input, ""));
    let physical = KeyCode::from_name(physical);
    let layout = match KeyCode::from_name(layout) {
        Some(code) => code,
        None if physical.is_some() => KeyCode::Unknown,
        None => return 0,
    };
    let mods = KeyMods {
        shift: modifiers & KeyMods::SHIFT != 0,
        ctrl: modifiers & KeyMods::CTRL != 0,
        alt: modifiers & KeyMods::ALT != 0,
        super_key: modifiers & KeyMods::SUPER != 0,
    };
    let key = match physical {
        Some(code) => KeyPress::from_layout(layout, code, mods),
        None => KeyPress::new(layout, mods),
    };
    RUNNER.with_borrow_mut(|runner| {
        if runner.core_mut().interaction.modifiers() != mods {
            runner.input(InputEvent::Modifiers(mods), time);
        }
        runner.key_event(key, time, down != 0);
    });
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_files(kind: u32, x: f32, y: f32, time: f64) {
    let paths = serde_json::from_str(&browser_text()).unwrap_or_default();
    let at = Vec2::new(x, y);
    let event = match kind {
        0 => InputEvent::Files(paths),
        1 => InputEvent::DragFiles { paths, at },
        2 => InputEvent::DropFiles { paths, at },
        _ => InputEvent::DragCancel,
    };
    RUNNER.with_borrow_mut(|runner| runner.input(event, time));
}
