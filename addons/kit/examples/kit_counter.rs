//! A standalone Rust host: only kui-native and kui-kit, with no Lua files.
//!
//! Run: cargo run -p kui-kit --example kit_counter
//! Test: cargo test -p kui-kit --example kit_counter
//! Copy this file to src/main.rs of a project depending on those two crates.

use kui_kit as kit;
use kui_native::{App, Message, NodeSpec, Ui, UiEvent};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Increment,
}

#[derive(Default)]
struct Counter {
    count: u32,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill().pad(24.0).gap(12.0));
        kit::Settings::new().skin(kit::Skin::Neutral).place(ui);
        kit::Heading::new(format!("Count: {}", self.count)).place(ui);
        kit::Button::new("Increment")
            .variant(kit::Variant::Primary)
            .on_press(Msg::Increment)
            .place(ui);
        // This preset also exercises the WGSL embedded in Kit's Lua.
        kit::Shader::new()
            .preset(kit::ShaderPreset::Noise)
            .height(48.0)
            .label("Kit's embedded shader")
            .place(ui);
    }

    fn on_event(&mut self, event: UiEvent) {
        if event.message::<Msg>() == Some(Msg::Increment) {
            self.count += 1;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Kit from Rust")
        .size(360.0, 240.0)
        .extension_as(kit::NAMESPACE, kit::extension()?)
        .run(Counter::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_native::{NodeKind, testing::Drive};

    #[test]
    fn embedded_kit_draws_and_delivers_a_typed_rust_message() {
        let mut app = Counter::default();
        let mut drive = Drive::new(kui_native::system::new_core(), 360.0, 240.0).framing();
        drive
            .extension(kit::NAMESPACE, kit::extension().unwrap())
            .unwrap();
        drive.frame(&mut app);
        let button = drive.key_named("Increment").expect("Kit button rendered");
        drive.click_key(&mut app, button);
        assert_eq!(app.count, 1, "Kit's event reached the Rust host");
        assert!(
            drive.key_named("Count: 1").is_some(),
            "updated Rust state rendered"
        );
        assert!(
            drive
                .core
                .nodes()
                .into_iter()
                .any(|node| node.kind == NodeKind::Fragment),
            "Kit's embedded shader rendered"
        );
        let warnings = drive.warnings();
        assert!(warnings.is_empty(), "{warnings:?}");
    }
}
