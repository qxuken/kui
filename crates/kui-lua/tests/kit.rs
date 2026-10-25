//! Kit (addons/kit) from a Rust host, through the wrappers its schema
//! generates (ADR 0066): it provides every component the schema
//! describes, each draws without a warning, its events reach the host as
//! the host's own messages, and a skin switch re-asks every instance.

use kui_core::channel::{Channel, Column};
use kui_core::testing::click_at;
use kui_core::{Core, EventKind, Extension, Extensions, NodeSpec, Size, SlotFill, Ui, UiEvent, Value};
use kui_kit as kit;
use kui_lua::LuaExtension;

const KIT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../addons/kit/lua/kit.lua");
const SCHEMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../addons/kit/schema.lua");
const VIEW: Size = Size { w: 1200.0, h: 900.0 };

fn msg(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

struct Rig {
    core: Core,
    exts: Extensions,
}

fn rig() -> Rig {
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(LuaExtension::from_file(KIT).expect("load Kit")))
        .unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    Rig { core, exts }
}

impl Rig {
    fn frame(&mut self, view: impl FnOnce(&mut Ui<'_>)) {
        let mut ui = self.core.frame_with(VIEW, 1.0, &mut self.exts);
        ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(6.0));
        view(&mut ui);
        ui.finish();
    }

    /// The centre of the first node whose accessible name is `label`.
    fn centre(&mut self, label: &str) -> (f32, f32) {
        let n = self
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some(label))
            .unwrap_or_else(|| panic!("no node named {label}"));
        (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
    }

    fn click(&mut self, label: &str) -> Vec<UiEvent> {
        let (x, y) = self.centre(label);
        let events = click_at(&mut self.core, x, y);
        let mut host = Vec::new();
        self.exts.route(events, |ev| host.push(ev));
        host
    }

    fn texts(&self) -> Vec<String> {
        self.core.nodes().into_iter().filter_map(|n| n.text).collect()
    }
}

fn rows() -> Channel {
    Channel::table(vec![
        ("name".into(), Column::strs(["alpha", "beta", "gamma"])),
        ("ms".into(), Column::F64(vec![12.5, 3.0, 40.25])),
        ("t".into(), Column::F64(vec![0.0, 1.0, 2.0])),
    ])
    .unwrap()
}

fn spans() -> Channel {
    Channel::table(vec![
        ("name".into(), Column::strs(["load", "parse", "paint"])),
        ("start".into(), Column::F64(vec![0.0, 2.0, 30.0])),
        ("len".into(), Column::F64(vec![60.0, 20.0, 10.0])),
        ("depth".into(), Column::I64(vec![0, 1, 1])),
        ("group".into(), Column::I64(vec![0, 1, 2])),
        ("target".into(), Column::strs(["a", "b", "c"])),
    ])
    .unwrap()
}

fn logs() -> Channel {
    Channel::table(vec![
        ("time".into(), Column::strs(["12:00:01", "12:00:02"])),
        ("level".into(), Column::strs(["info", "error"])),
        ("message".into(), Column::strs(["started", "boom"])),
    ])
    .unwrap()
}

/// One of every component, through the wrappers.
fn gallery(ui: &mut Ui<'_>, answers: &mut Vec<(&'static str, Option<SlotFill>)>) {
    let choices = vec![kit::Choice::new("a", "Alpha"), kit::Choice::new("b", "Beta")];
    let mut put = |name: &'static str, a: Option<SlotFill>| answers.push((name, a));
    put("settings", kit::Settings::new().place(ui));
    put("toolbar", kit::Toolbar::new().place_with(ui, |group, ui| {
        if group.is_empty() {
            kit::Button::new("Run").variant(kit::Variant::Primary).on_press(msg("run")).place(ui);
        } else {
            kit::Search::new("").on_change(msg("query")).place(ui);
        }
    }));
    put("panel", kit::Panel::new().title("Requests").place_with(ui, |group, ui| {
        if group.is_empty() {
            kit::Table::new(
                vec![
                    kit::Column::new("name", "Name"),
                    kit::Column::new("ms", "Time").align(kit::Align::End).mono(true).width(80.0),
                ],
                rows(),
            )
            .selected(1)
            .on_select(msg("pick"))
            .place(ui);
        }
    }));
    put("icon_button", kit::IconButton::new("refresh", "Reload").on_press(msg("reload")).place(ui));
    put("text_field", kit::TextField::new("hello").label("Name").place(ui));
    put("select", kit::Select::new(choices.clone()).value("a").label("Mode").place(ui));
    put("checkbox", kit::Checkbox::new("Follow", true).on_toggle(msg("follow")).place(ui));
    put("switch", kit::Switch::new("Live", false).on_toggle(msg("live")).place(ui));
    put("radio_group", kit::RadioGroup::new(choices.clone()).value("b").place(ui));
    put("segmented", kit::Segmented::new(choices.clone(), "a").on_change(msg("seg")).place(ui));
    put("slider", kit::Slider::new(0.3, 0.0, 1.0).label("Zoom").place(ui));
    put("split", kit::Split::new(0.4).place_with(ui, |group, ui| {
        kit::Badge::new(group).place(ui);
    }));
    put("tabs", kit::Tabs::new(vec![kit::Tab::new("a", "One"), kit::Tab::new("b", "Two").count(3)], "a")
        .place_with(ui, |_, ui| kit::Divider::new().place(ui).map(|_| ()).unwrap_or(())));
    put("section", kit::Section::new("More").count(2).place_with(ui, |_, ui| {
        kit::Code::new("let x = 1;").place(ui);
    }));
    put("list", kit::List::new(rows(), "name").detail("ms").place(ui));
    put("inspector", kit::Inspector::new(vec![kit::Entry::new("status", "200").mono(true)]).place(ui));
    put("log", kit::Log::new(logs()).place(ui));
    put("stat", kit::Stat::new("p95", "412 ms").delta("-8%").tone(kit::Tone::Ok).place(ui));
    put("empty", kit::Empty::new("Nothing yet").detail("Run a request.").place(ui));
    put("progress", kit::Progress::new().value(0.4).label("Loading").place(ui));
    put("notice", kit::Notice::new("The port is busy.").tone(kit::Tone::Danger).title("Not bound").place_with(ui, |_, ui| {
        kit::Button::new("Retry").place(ui);
    }));
    put("pressed", kit::Button::new("Logs").pressed(true).icon("terminal").tooltip("Hide the log").place(ui));
    put("sparkline", kit::Sparkline::new(rows(), "ms").place(ui));
    put("line_chart", kit::LineChart::new(rows(), "t", vec![kit::Series::new("ms", "Time")]).area(true).place(ui));
    put("bar_chart", kit::BarChart::new(rows(), "name", "ms").place(ui));
    put("histogram", kit::Histogram::new(rows(), "ms").bins(4).place(ui));
    put("waterfall", kit::Waterfall::new(spans()).place(ui));
    put("statusbar", kit::Statusbar::new().place_with(ui, |_, ui| {
        kit::Badge::new("connected").tone(kit::Tone::Ok).place(ui);
    }));
}

#[test]
fn kit_provides_every_component_its_schema_describes() {
    let schema = kui_addon_schema::Schema::from_path(std::path::Path::new(SCHEMA)).unwrap();
    let mut want: Vec<String> = schema.components.iter().map(|c| c.name.clone()).collect();
    want.sort();
    let mut have = LuaExtension::from_file(KIT).unwrap().components().to_vec();
    have.sort();
    assert_eq!(have, want);
}

#[test]
fn every_component_draws_without_a_warning() {
    let mut r = rig();
    let mut answers = Vec::new();
    r.frame(|ui| gallery(ui, &mut answers));
    let unanswered: Vec<_> = answers.iter().filter(|(_, a)| a.is_none()).map(|(n, _)| *n).collect();
    assert!(unanswered.is_empty(), "nothing provided {unanswered:?}");
    let warnings = r.core.take_warnings();
    assert!(warnings.is_empty(), "{warnings:#?}");
    // A second frame with nothing moved replays what read nothing that moved.
    let mut again = Vec::new();
    r.frame(|ui| gallery(ui, &mut again));
    let replayed = again.iter().filter(|(_, a)| *a == Some(SlotFill::Replayed)).count();
    assert!(replayed >= 10, "{again:?}");
    assert!(r.core.take_warnings().is_empty());
}

#[test]
fn a_press_reaches_the_host_as_its_own_message() {
    let mut r = rig();
    let view = |ui: &mut Ui<'_>| {
        kit::Button::new("Run").on_press(msg("run")).place(ui);
        kit::Checkbox::new("Follow", false).on_toggle(msg("follow")).place(ui);
        kit::Segmented::new(vec![kit::Choice::new("a", "Alpha"), kit::Choice::new("b", "Beta")], "a")
            .on_change(msg("seg"))
            .place(ui);
    };
    r.frame(view);
    r.frame(view);
    let ev = r.click("Run");
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0].event, EventKind::Component);
    assert_eq!(ev[0].payload.get_str("kind"), Some("run"));
    assert!(kit::ButtonPress::from_payload(&ev[0].payload).is_some());

    let ev = r.click("Follow");
    let toggle = kit::CheckboxToggle::from_payload(&ev[0].payload).expect("a toggle");
    assert!(toggle.checked);

    let ev = r.click("Beta");
    assert_eq!(kit::SegmentedChange::from_payload(&ev[0].payload).unwrap().value, "b");
}

#[test]
fn a_select_opens_picks_and_closes_on_its_own() {
    let mut r = rig();
    let view = |ui: &mut Ui<'_>| {
        kit::Select::new(vec![kit::Choice::new("a", "Alpha"), kit::Choice::new("b", "Beta")])
            .value("a")
            .label("Mode")
            .on_change(msg("mode"))
            .place(ui);
    };
    r.frame(view);
    r.frame(view);
    assert!(r.click("Mode: Alpha").is_empty(), "opening is Kit's own business");
    r.frame(view);
    assert!(r.texts().iter().any(|t| t == "Beta"), "open: {:?}", r.texts());
    let ev = r.click("Beta");
    assert_eq!(kit::SelectChange::from_payload(&ev[0].payload).unwrap().value, "b");
    r.frame(view);
    assert!(!r.texts().iter().any(|t| t == "Beta"), "closed after the pick");
}

#[test]
fn a_skin_switch_reasks_every_instance() {
    let mut r = rig();
    let view = |skin: kit::Skin| {
        move |ui: &mut Ui<'_>| {
            kit::Settings::new().skin(skin).place(ui);
            kit::Button::new("Save").place(ui);
        }
    };
    r.frame(view(kit::Skin::Ledger));
    r.frame(view(kit::Skin::Ledger));
    assert!(r.texts().contains(&"SAVE".to_string()), "Ledger labels in capitals");
    r.frame(view(kit::Skin::Neutral));
    assert!(
        r.texts().contains(&"Save".to_string()),
        "the button was asked again under Neutral: {:?}",
        r.texts()
    );
}

#[test]
fn kit_components_nest_inside_kit_components_through_the_hosts_children() {
    let mut r = rig();
    let view = |ui: &mut Ui<'_>| {
        kit::Panel::new().title("Outer").place_with(ui, |group, ui| {
            if group.is_empty() {
                kit::Toolbar::new().place_with(ui, |group, ui| {
                    if group.is_empty() {
                        kit::Button::new("Deep").on_press(msg("deep")).place(ui);
                    }
                });
            }
        });
    };
    r.frame(view);
    assert!(r.core.take_warnings().is_empty());
    r.frame(view);
    let ev = r.click("Deep");
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0].payload.get_str("kind"), Some("deep"));
}

#[test]
fn hundreds_of_host_children_keep_keys_of_their_own() {
    // The 84th child of the unnamed group once had the key of the
    // toolbar's first node (Key::str("") XORed its tag away).
    let mut r = rig();
    let view = |ui: &mut Ui<'_>| {
        kit::Toolbar::new().place_with(ui, |group, ui| {
            if group.is_empty() {
                for i in 0..200 {
                    kit::Button::new(format!("B{i}")).place(ui);
                }
            }
        });
    };
    r.frame(view);
    r.frame(view);
    let w = r.core.take_warnings();
    assert!(w.is_empty(), "{w:#?}");
}

#[test]
fn a_lua_host_puts_kit_inside_kit_and_kit_draws_it() {
    // A Lua host's children are built while Kit's toolbar builds: the
    // button they name is still the host's to place, through the list, so
    // Kit draws it under Kit's origin with Kit's tokens.
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(LuaExtension::from_file(KIT).unwrap()))
        .unwrap();
    exts.push_as(
        "app",
        Box::new(
            LuaExtension::from_source(
                "app.lua",
                r#"
                function view(env)
                  return column { width = "grow",
                    component("kit/toolbar", {}, { component("kit/button", { label = "Go" }) }) }
                end
                "#,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame_with(VIEW, 1.0, &mut exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.finish();
        let w = core.take_warnings();
        assert!(w.is_empty(), "{w:#?}");
    }
    let go = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Go"))
        .map(|n| n.origin)
        .expect("the button drew");
    assert_eq!(go, kui_core::OriginId(1), "Kit's own origin");
}
