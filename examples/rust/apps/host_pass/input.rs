use kui_access::HostBridge;
use kui_core::{
    Core, InputEvent, KeyCode, KeyLocation, KeyMods, KeyPress, MouseButton, UiEvent, Vec2,
};
use winit::event::{
    ElementState, Ime, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent,
};
use winit::keyboard::{
    Key, KeyLocation as WinitKeyLocation, ModifiersState, NamedKey, PhysicalKey,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::game::Game;

#[derive(Default)]
pub struct Input {
    cursor: Vec2,
    modifiers: KeyMods,
}

pub struct Outcome {
    pub events: Vec<UiEvent>,
    pub close: bool,
}

impl Input {
    pub fn handle(
        &mut self,
        core: &mut Core,
        game: &mut Game,
        event: &WindowEvent,
        scale: f32,
    ) -> Outcome {
        let mut close = false;
        let events = match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Vec2::new(position.x as f32 / scale, position.y as f32 / scale);
                core.handle_input(InputEvent::CursorMoved(self.cursor))
            }
            WindowEvent::CursorLeft { .. } => core.handle_input(InputEvent::CursorLeft),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = mouse_button(*button);
                if *state == ElementState::Pressed
                    && button == MouseButton::Primary
                    && !core.pointer_wants()
                {
                    game.world_click(self.cursor.x, self.cursor.y);
                }
                match state {
                    ElementState::Pressed => {
                        core.handle_input(InputEvent::MouseDown { button, clicks: 1 })
                    }
                    ElementState::Released => core.handle_input(InputEvent::MouseUp { button }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(x, y) => core.handle_input(InputEvent::Wheel {
                    lines: Vec2::new(*x, *y),
                    begins: true,
                }),
                MouseScrollDelta::PixelDelta(p) => core.handle_input(InputEvent::Scroll(
                    Vec2::new(p.x as f32 / scale, p.y as f32 / scale),
                )),
            },
            WindowEvent::KeyboardInput { event, .. } => {
                let code = logical_code(&event.logical_key);
                let physical = physical_code(event.physical_key);
                let mut press = KeyPress::new(code, self.modifiers)
                    .with_physical(physical)
                    .with_location(key_location(event.location));
                press.repeat = event.repeat;
                if let Some(text) = &event.text {
                    press = press.with_text(text.as_str());
                }
                if event.state == ElementState::Pressed && !core.keyboard_wants() {
                    match code {
                        KeyCode::Escape => close = true,
                        KeyCode::Space => game.harvest(),
                        _ => {}
                    }
                }
                match event.state {
                    ElementState::Pressed => core.handle_input(InputEvent::KeyDown(press)),
                    ElementState::Released => core.handle_input(InputEvent::KeyUp(press)),
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                core.handle_input(InputEvent::Commit(text.clone()))
            }
            WindowEvent::Ime(Ime::Preedit(text, cursor)) => {
                core.handle_input(InputEvent::Preedit(text.clone(), *cursor))
            }
            WindowEvent::Ime(Ime::Disabled) => {
                core.handle_input(InputEvent::Preedit(String::new(), None))
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = key_mods(modifiers.state());
                core.handle_input(InputEvent::Modifiers(self.modifiers))
            }
            WindowEvent::Focused(focused) => {
                core.set_focused(*focused);
                Vec::new()
            }
            _ => Vec::new(),
        };
        Outcome { events, close }
    }
}

fn mouse_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Primary,
        WinitMouseButton::Right => MouseButton::Secondary,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Other(0),
        WinitMouseButton::Forward => MouseButton::Other(1),
        WinitMouseButton::Other(n) => MouseButton::Other((n + 2).min(u8::MAX as u16) as u8),
    }
}

fn key_mods(modifiers: ModifiersState) -> KeyMods {
    let mut mods = KeyMods::NONE;
    if modifiers.shift_key() {
        mods = mods.with_shift();
    }
    if modifiers.control_key() {
        mods = mods.with_ctrl();
    }
    if modifiers.alt_key() {
        mods = mods.with_alt();
    }
    if modifiers.super_key() {
        mods = mods.with_super();
    }
    mods
}

fn key_location(location: WinitKeyLocation) -> KeyLocation {
    match location {
        WinitKeyLocation::Standard => KeyLocation::Standard,
        WinitKeyLocation::Left => KeyLocation::Left,
        WinitKeyLocation::Right => KeyLocation::Right,
        WinitKeyLocation::Numpad => KeyLocation::Numpad,
    }
}

fn logical_code(key: &Key) -> KeyCode {
    match key {
        Key::Character(text) => text.chars().next().map_or(KeyCode::Unknown, KeyCode::Char),
        Key::Named(named) => match named {
            NamedKey::ArrowLeft => KeyCode::Left,
            NamedKey::ArrowRight => KeyCode::Right,
            NamedKey::ArrowUp => KeyCode::Up,
            NamedKey::ArrowDown => KeyCode::Down,
            NamedKey::Home => KeyCode::Home,
            NamedKey::End => KeyCode::End,
            NamedKey::PageUp => KeyCode::PageUp,
            NamedKey::PageDown => KeyCode::PageDown,
            NamedKey::Backspace => KeyCode::Backspace,
            NamedKey::Delete => KeyCode::Delete,
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Tab => KeyCode::Tab,
            NamedKey::Escape => KeyCode::Escape,
            NamedKey::Space => KeyCode::Space,
            NamedKey::Insert => KeyCode::Insert,
            NamedKey::PrintScreen => KeyCode::PrintScreen,
            NamedKey::Pause => KeyCode::Pause,
            NamedKey::ContextMenu => KeyCode::Menu,
            NamedKey::Shift => KeyCode::Shift,
            NamedKey::Control => KeyCode::Ctrl,
            NamedKey::Alt => KeyCode::Alt,
            NamedKey::Super => KeyCode::Super,
            NamedKey::CapsLock => KeyCode::CapsLock,
            NamedKey::NumLock => KeyCode::NumLock,
            NamedKey::ScrollLock => KeyCode::ScrollLock,
            NamedKey::F1 => KeyCode::F(1),
            NamedKey::F2 => KeyCode::F(2),
            NamedKey::F3 => KeyCode::F(3),
            NamedKey::F4 => KeyCode::F(4),
            NamedKey::F5 => KeyCode::F(5),
            NamedKey::F6 => KeyCode::F(6),
            NamedKey::F7 => KeyCode::F(7),
            NamedKey::F8 => KeyCode::F(8),
            NamedKey::F9 => KeyCode::F(9),
            NamedKey::F10 => KeyCode::F(10),
            NamedKey::F11 => KeyCode::F(11),
            NamedKey::F12 => KeyCode::F(12),
            _ => KeyCode::Unknown,
        },
        _ => KeyCode::Unknown,
    }
}

fn physical_code(key: PhysicalKey) -> KeyCode {
    let PhysicalKey::Code(code) = key else {
        return KeyCode::Unknown;
    };
    use winit::keyboard::KeyCode as W;
    match code {
        W::KeyA => KeyCode::Char('a'),
        W::KeyB => KeyCode::Char('b'),
        W::KeyC => KeyCode::Char('c'),
        W::KeyD => KeyCode::Char('d'),
        W::KeyE => KeyCode::Char('e'),
        W::KeyF => KeyCode::Char('f'),
        W::KeyG => KeyCode::Char('g'),
        W::KeyH => KeyCode::Char('h'),
        W::KeyI => KeyCode::Char('i'),
        W::KeyJ => KeyCode::Char('j'),
        W::KeyK => KeyCode::Char('k'),
        W::KeyL => KeyCode::Char('l'),
        W::KeyM => KeyCode::Char('m'),
        W::KeyN => KeyCode::Char('n'),
        W::KeyO => KeyCode::Char('o'),
        W::KeyP => KeyCode::Char('p'),
        W::KeyQ => KeyCode::Char('q'),
        W::KeyR => KeyCode::Char('r'),
        W::KeyS => KeyCode::Char('s'),
        W::KeyT => KeyCode::Char('t'),
        W::KeyU => KeyCode::Char('u'),
        W::KeyV => KeyCode::Char('v'),
        W::KeyW => KeyCode::Char('w'),
        W::KeyX => KeyCode::Char('x'),
        W::KeyY => KeyCode::Char('y'),
        W::KeyZ => KeyCode::Char('z'),
        W::Digit0 => KeyCode::Char('0'),
        W::Digit1 => KeyCode::Char('1'),
        W::Digit2 => KeyCode::Char('2'),
        W::Digit3 => KeyCode::Char('3'),
        W::Digit4 => KeyCode::Char('4'),
        W::Digit5 => KeyCode::Char('5'),
        W::Digit6 => KeyCode::Char('6'),
        W::Digit7 => KeyCode::Char('7'),
        W::Digit8 => KeyCode::Char('8'),
        W::Digit9 => KeyCode::Char('9'),
        W::Space => KeyCode::Space,
        W::Escape => KeyCode::Escape,
        W::Enter => KeyCode::Enter,
        W::Tab => KeyCode::Tab,
        W::Backspace => KeyCode::Backspace,
        W::Delete => KeyCode::Delete,
        W::ArrowLeft => KeyCode::Left,
        W::ArrowRight => KeyCode::Right,
        W::ArrowUp => KeyCode::Up,
        W::ArrowDown => KeyCode::Down,
        W::Home => KeyCode::Home,
        W::End => KeyCode::End,
        W::PageUp => KeyCode::PageUp,
        W::PageDown => KeyCode::PageDown,
        _ => KeyCode::Unknown,
    }
}

pub fn attach_access(window: &Window) -> Option<HostBridge> {
    #[cfg(target_os = "macos")]
    {
        let raw = window.window_handle().ok()?.as_raw();
        let RawWindowHandle::AppKit(handle) = raw else {
            return None;
        };
        unsafe { HostBridge::attach(handle.ns_view.as_ptr()) }
    }
    #[cfg(target_os = "windows")]
    {
        let raw = window.window_handle().ok()?.as_raw();
        let RawWindowHandle::Win32(handle) = raw else {
            return None;
        };
        unsafe { HostBridge::attach(handle.hwnd.as_ptr()) }
    }
    #[cfg(target_os = "linux")]
    {
        unsafe { HostBridge::attach(std::ptr::null_mut()) }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}
