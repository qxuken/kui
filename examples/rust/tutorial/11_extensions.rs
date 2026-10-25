//! Step 11 — extensions. A Rust host loads two Lua scripts and lets them
//! draw inside its frame: it places the notes plugin's list, a component,
//! where it wants it, and declares a toolbar slot both plugins add a button
//! to. What they raise comes back as the host's own messages. Chapter 13
//! of the book.
//!
//! Run: cargo run -p kui-lua --example tutorial_11_extensions
//! Test: cargo test -p kui-lua --example tutorial_11_extensions

use kui_lua::LuaExtension;
use kui_native::{Align, App, Message, NodeSpec, TextStyle, Ui, UiEvent, Value};

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    /// A note was picked in the notes plugin's list.
    Picked,
    /// A toolbar button ran — whichever plugin's.
    Ran,
}
// ANCHOR_END: message

#[derive(Default)]
struct Notes {
    picked: Option<String>,
    log: Vec<String>,
    /// Bumped when a toolbar button ran: the list's props change, so the
    /// list is drawn again rather than replayed.
    revision: i64,
}

impl App for Notes {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.configure_root(NodeSpec::column().fill().pad(16.0).gap(12.0).bg(t.bg));

        // ANCHOR: slot
        // Whoever contributes to `app.toolbar` puts a button here, after
        // the title. The host does not know who will.
        ui.with_keyed(
            "toolbar",
            NodeSpec::row().gap(8.0).cross_align(Align::Center),
            |ui| {
                ui.text("Notes", TextStyle::new(18.0).color(t.fg));
                ui.slot("app.toolbar", &Value::map([("onRun", Msg::Ran.into())]));
            },
        );
        // ANCHOR_END: slot

        // ANCHOR: component
        // The notes plugin's list, placed by name, with props in and the
        // host's own message for its `onPick`.
        ui.component(
            "notes/list",
            &Value::map([
                ("title", "Today".into()),
                ("revision", Value::Int(self.revision)),
                ("onPick", Msg::Picked.into()),
            ]),
        );
        // ANCHOR_END: component

        let picked = self.picked.as_deref().unwrap_or("nothing");
        let last = self.log.last().map_or("", String::as_str);
        ui.text_in_keyed(
            "status",
            NodeSpec::row(),
            &format!("picked {picked} · {last}"),
            TextStyle::new(13.0).color(t.muted),
        );
    }

    // ANCHOR: on_event
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            // The plugin's fields arrive in the event's field: `pick` for
            // `onPick`, `run` for `onRun`.
            Some(Msg::Picked) => {
                self.picked = ev.data().and_then(|d| d.get_str("title")).map(String::from);
            }
            // Every toolbar button raises the one message; `from` says
            // which plugin did.
            Some(Msg::Ran) => {
                let action = ev.data().and_then(|d| d.get_str("action")).unwrap_or("?");
                let who = ev.from.as_deref().unwrap_or("?");
                self.log.push(format!("{who}: {action}"));
                self.revision += 1;
            }
            None => {}
        }
    }
    // ANCHOR_END: on_event
}

// ANCHOR: main
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let notes = LuaExtension::from_source("notes.lua", include_str!("11_extensions/notes.lua"))?;
    let stamp = LuaExtension::from_source("stamp.lua", include_str!("11_extensions/stamp.lua"))?;
    kui_native::app("Notes")
        .size(480.0, 340.0)
        .extension_as("notes", notes)
        .extension_as("stamp", stamp)
        .run(Notes::default())
}
// ANCHOR_END: main

// ANCHOR: tests
#[cfg(test)]
mod tests {
    use super::*;
    use kui_native::Core;
    use kui_native::testing::Drive;

    fn drive() -> Drive {
        let mut d = Drive::new(Core::new(), 480.0, 340.0).framing();
        let notes = include_str!("11_extensions/notes.lua");
        let stamp = include_str!("11_extensions/stamp.lua");
        d.extension(
            "notes",
            LuaExtension::from_source("notes.lua", notes).unwrap(),
        )
        .unwrap();
        d.extension(
            "stamp",
            LuaExtension::from_source("stamp.lua", stamp).unwrap(),
        )
        .unwrap();
        d
    }

    #[test]
    fn both_plugins_add_to_the_toolbar_and_the_host_hears_who() {
        let mut app = Notes::default();
        let mut d = drive();
        d.frame(&mut app);
        // `stamp` asked for `order = -1`, so it goes first.
        assert_eq!(d.texts_under("toolbar"), ["Notes", "Stamp (0)", "New note"]);

        let new = d.key_of("New note").expect("the notes plugin's button");
        d.click_key(&mut app, new);
        assert_eq!(app.log, ["notes: new note"]);
        assert!(d.texts_under("list").contains(&"Note 4".to_string()));

        let stamp = d.key_of("Stamp (0)").expect("the stamp plugin's button");
        d.click_key(&mut app, stamp);
        assert_eq!(app.log.last().map(String::as_str), Some("stamp: stamp"));
        assert!(d.texts_under("toolbar").contains(&"Stamp (1)".to_string()));
        assert!(d.warnings().is_empty(), "{:?}", d.warnings());
    }

    #[test]
    fn a_picked_note_comes_back_as_the_hosts_message() {
        let mut app = Notes::default();
        let mut d = drive();
        d.frame(&mut app);
        let note = d.key_of("Call the dentist").expect("a row of the list");
        d.click_key(&mut app, note);
        assert_eq!(app.picked.as_deref(), Some("Call the dentist"));
        assert_eq!(d.texts_under("status"), ["picked Call the dentist · "]);
    }
}
// ANCHOR_END: tests
