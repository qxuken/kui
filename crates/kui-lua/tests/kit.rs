//! Kit (addons/kit) from a Rust host, through the wrappers its schema
//! generates (ADR 0066): it provides every component the schema
//! describes, each draws without a warning, its events reach the host as
//! the host's own messages, and a skin switch re-asks every instance.

use kui_core::channel::{Channel, Column};
use kui_core::testing::click_at;
use kui_core::{
    Core, EventKind, Extension, Extensions, FillAnswer, NodeSpec, Size, Ui, UiEvent, Value,
};
use kui_kit as kit;
use kui_lua::LuaExtension;

const KIT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../addons/kit/lua/kit.lua");
const SCHEMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../addons/kit/schema.lua");
const VIEW: Size = Size {
    w: 1200.0,
    h: 900.0,
};

fn msg(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

struct Rig {
    core: Core,
    exts: Extensions,
}

fn rig() -> Rig {
    let mut exts = Extensions::new();
    exts.push_as(
        "kit",
        Box::new(LuaExtension::from_file(KIT).expect("load Kit")),
    )
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
        self.core
            .nodes()
            .into_iter()
            .filter_map(|n| n.text)
            .collect()
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

fn states() -> Channel {
    Channel::table(vec![
        ("name".into(), Column::strs(["auth", "orders", "search"])),
        ("state".into(), Column::strs(["healthy", "down", "GET"])),
        ("tone".into(), Column::strs(["ok", "danger", "accent"])),
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
fn gallery(ui: &mut Ui<'_>, answers: &mut Vec<(&'static str, Option<FillAnswer>)>) {
    let choices = vec![
        kit::Choice::new("a", "Alpha"),
        kit::Choice::new("b", "Beta"),
    ];
    let mut put = |name: &'static str, a: Option<FillAnswer>| answers.push((name, a));
    put("settings", kit::Settings::new().place(ui));
    put(
        "toolbar",
        kit::Toolbar::new().place_with(ui, |group, ui| {
            if group.is_empty() {
                kit::Button::new("Run")
                    .variant(kit::Variant::Primary)
                    .on_press(msg("run"))
                    .place(ui);
            } else {
                kit::Search::new("").on_change(msg("query")).place(ui);
            }
        }),
    );
    put(
        "panel",
        kit::Panel::new()
            .title("Requests")
            .place_with(ui, |group, ui| {
                if group.is_empty() {
                    kit::Table::new(
                        vec![
                            kit::Column::new("name", "Name"),
                            kit::Column::new("ms", "Time")
                                .align(kit::Align::End)
                                .mono(true)
                                .width(80.0),
                        ],
                        rows(),
                    )
                    .selected(1)
                    .on_select(msg("pick"))
                    .place(ui);
                }
            }),
    );
    put(
        "icon_button",
        kit::IconButton::new("refresh", "Reload")
            .on_press(msg("reload"))
            .place(ui),
    );
    put(
        "text_field",
        kit::TextField::new("hello").label("Name").place(ui),
    );
    put(
        "text_field_error",
        kit::TextField::new("a/b")
            .label("Device")
            .error("A name cannot hold /.")
            .place(ui),
    );
    put(
        "text_area",
        kit::TextArea::new("line one\nline two")
            .label("Note")
            .rows(3)
            .max_rows(6)
            .place(ui),
    );
    put(
        "table_toned",
        kit::Table::new(
            vec![
                kit::Column::new("name", "Name"),
                kit::Column::new("state", "State")
                    .tone("tone")
                    .badge(true)
                    .width(90.0),
            ],
            states(),
        )
        .place(ui),
    );
    put(
        "list_badged",
        kit::List::new(states(), "name")
            .badge("state")
            .tone("tone")
            .badge_width(60.0)
            .place(ui),
    );
    put(
        "select",
        kit::Select::new(choices.clone())
            .value("a")
            .label("Mode")
            .place(ui),
    );
    put(
        "checkbox",
        kit::Checkbox::new("Follow", true)
            .on_toggle(msg("follow"))
            .place(ui),
    );
    put(
        "checkbox_rich",
        kit::Checkbox::new("mobile-limit", false)
            .hide_label(true)
            .grow(true)
            .place_with(ui, |_, ui| {
                ui.text("mobile-limit", kui_core::TextStyle::new(12.0));
            }),
    );
    put(
        "switch",
        kit::Switch::new("Live", false)
            .on_toggle(msg("live"))
            .place(ui),
    );
    put(
        "radio_group",
        kit::RadioGroup::new(choices.clone()).value("b").place(ui),
    );
    put(
        "radio_group_unlabelled",
        kit::RadioGroup::new(choices.clone())
            .value("a")
            .label("Theme")
            .hide_label(true)
            .place(ui),
    );
    put(
        "segmented",
        kit::Segmented::new(choices.clone(), "a")
            .label("Filter")
            .on_change(msg("seg"))
            .place(ui),
    );
    put(
        "slider",
        kit::Slider::new(0.3, 0.0, 1.0).label("Zoom").place(ui),
    );
    put(
        "slider_said",
        kit::Slider::new(2500.0, 0.0, 10000.0)
            .label("Upload")
            .value_label("2.5 MB/s")
            .place(ui),
    );
    put(
        "split",
        kit::Split::new(0.4).place_with(ui, |group, ui| {
            kit::Badge::new(group).place(ui);
        }),
    );
    put(
        "tabs",
        kit::Tabs::new(
            vec![
                kit::Tab::new("a", "One"),
                kit::Tab::new("b", "Two").count(3),
            ],
            "a",
        )
        .place_with(ui, |_, ui| {
            kit::Divider::new().place(ui).map(|_| ()).unwrap_or(())
        }),
    );
    put(
        "section",
        kit::Section::new("More").count(2).place_with(ui, |_, ui| {
            kit::Code::new("let x = 1;").place(ui);
        }),
    );
    put(
        "list",
        kit::List::new(rows(), "name").detail("ms").place(ui),
    );
    put(
        "inspector",
        kit::Inspector::new(vec![kit::Entry::new("status", "200").mono(true)]).place(ui),
    );
    put("log", kit::Log::new(logs()).place(ui));
    put(
        "log_wrapped",
        kit::Log::new(logs()).wrap(true).time(false).place(ui),
    );
    put(
        "stat",
        kit::Stat::new("p95", "412 ms")
            .delta("-8%")
            .tone(kit::Tone::Ok)
            .place(ui),
    );
    put(
        "empty",
        kit::Empty::new("Nothing yet")
            .detail("Run a request.")
            .place(ui),
    );
    put(
        "progress",
        kit::Progress::new().value(0.4).label("Loading").place(ui),
    );
    put(
        "notice",
        kit::Notice::new("The port is busy.")
            .tone(kit::Tone::Danger)
            .title("Not bound")
            .place_with(ui, |_, ui| {
                kit::Button::new("Retry").place(ui);
            }),
    );
    put(
        "pressed",
        kit::Button::new("Logs")
            .pressed(true)
            .icon("terminal")
            .tooltip("Hide the log")
            .place(ui),
    );
    put("sparkline", kit::Sparkline::new(rows(), "ms").place(ui));
    put(
        "line_chart",
        kit::LineChart::new(rows(), "t", vec![kit::Series::new("ms", "Time")])
            .area(true)
            .place(ui),
    );
    put(
        "line_chart_ago",
        kit::LineChart::new(rows(), "t", vec![kit::Series::new("ms", "Time")])
            .x_format(kit::XFormat::Ago)
            .x_unit("min")
            .y_unit("ms")
            .place(ui),
    );
    put(
        "line_chart_clock",
        kit::LineChart::new(rows(), "t", vec![kit::Series::new("ms", "Time")])
            .x_format(kit::XFormat::Clock)
            .place(ui),
    );
    put(
        "bar_chart",
        kit::BarChart::new(rows(), "name", "ms").place(ui),
    );
    put(
        "bar_chart_lying",
        kit::BarChart::new(rows(), "name", "ms")
            .horizontal(true)
            .unit("ms")
            .place(ui),
    );
    put(
        "histogram",
        kit::Histogram::new(rows(), "ms").bins(4).place(ui),
    );
    put("waterfall", kit::Waterfall::new(spans()).place(ui));
    put(
        "statusbar",
        kit::Statusbar::new().place_with(ui, |_, ui| {
            kit::Badge::new("connected").tone(kit::Tone::Ok).place(ui);
        }),
    );
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
    let unanswered: Vec<_> = answers
        .iter()
        .filter(|(_, a)| a.is_none())
        .map(|(n, _)| *n)
        .collect();
    assert!(unanswered.is_empty(), "nothing provided {unanswered:?}");
    let warnings = r.core.take_warnings();
    assert!(warnings.is_empty(), "{warnings:#?}");
    // A second frame with nothing moved replays what read nothing that moved.
    let mut again = Vec::new();
    r.frame(|ui| gallery(ui, &mut again));
    let replayed = again
        .iter()
        .filter(|(_, a)| *a == Some(FillAnswer::Replayed))
        .count();
    assert!(replayed >= 10, "{again:?}");
    assert!(r.core.take_warnings().is_empty());
}

#[test]
fn a_press_reaches_the_host_as_its_own_message() {
    let mut r = rig();
    let view = |ui: &mut Ui<'_>| {
        kit::Button::new("Run").on_press(msg("run")).place(ui);
        kit::Checkbox::new("Follow", false)
            .on_toggle(msg("follow"))
            .place(ui);
        kit::Segmented::new(
            vec![
                kit::Choice::new("a", "Alpha"),
                kit::Choice::new("b", "Beta"),
            ],
            "a",
        )
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
    assert_eq!(
        kit::SegmentedChange::from_payload(&ev[0].payload)
            .unwrap()
            .value,
        "b"
    );
}

#[test]
fn a_select_opens_picks_and_closes_on_its_own() {
    let mut r = rig();
    let view = |ui: &mut Ui<'_>| {
        kit::Select::new(vec![
            kit::Choice::new("a", "Alpha"),
            kit::Choice::new("b", "Beta"),
        ])
        .value("a")
        .label("Mode")
        .on_change(msg("mode"))
        .place(ui);
    };
    r.frame(view);
    r.frame(view);
    assert!(
        r.click("Mode: Alpha").is_empty(),
        "opening is Kit's own business"
    );
    r.frame(view);
    assert!(
        r.texts().iter().any(|t| t == "Beta"),
        "open: {:?}",
        r.texts()
    );
    let ev = r.click("Beta");
    assert_eq!(
        kit::SelectChange::from_payload(&ev[0].payload)
            .unwrap()
            .value,
        "b"
    );
    r.frame(view);
    assert!(
        !r.texts().iter().any(|t| t == "Beta"),
        "closed after the pick"
    );
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
    assert!(
        r.texts().contains(&"SAVE".to_string()),
        "Ledger labels in capitals"
    );
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
        kit::Panel::new()
            .title("Outer")
            .place_with(ui, |group, ui| {
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

#[test]
fn a_child_left_out_with_nil_leaves_the_rest_in_place() {
    // `cond and node or nil` in the middle of a list: the children after
    // the hole are built, in a box and among a component's host children
    // alike — `#t` after the prelude's `t.type =` rehash stopped at the
    // first hole and dropped them without a word.
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
                    text("a"), false and text("x") or nil, nil, text("b"),
                    component("kit/toolbar", {}, { text("c"), nil, text("d") }),
                    text("e") }
                end
                "#,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    for _ in 0..2 {
        let mut ui = core.frame_with(VIEW, 1.0, &mut exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.finish();
    }
    let texts: Vec<String> = core.nodes().into_iter().filter_map(|n| n.text).collect();
    assert_eq!(texts, ["a", "b", "c", "d", "e"]);
}

/// A Lua app placing Kit, its view run `frames` times; the warnings the
/// frames raised, and the core to read afterwards.
fn lua_app(src: &str, frames: usize) -> (Core, Vec<String>) {
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(LuaExtension::from_file(KIT).unwrap()))
        .unwrap();
    exts.push_as(
        "app",
        Box::new(LuaExtension::from_source("app.lua", src).unwrap()),
    )
    .unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    let mut warnings = Vec::new();
    for _ in 0..frames {
        let mut ui = core.frame_with(VIEW, 1.0, &mut exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.finish();
        warnings.extend(core.take_warnings().into_iter().map(|w| format!("{w:?}")));
    }
    (core, warnings)
}

/// A Lua app placing Kit, kept across frames: its own `on_event` hears
/// what Kit proposes, so a test drives it with the pointer and reads the
/// tree it draws.
struct LuaRig {
    core: Core,
    exts: Extensions,
    warnings: Vec<String>,
    /// Events that reached no extension: the host's.
    host: Vec<UiEvent>,
    /// The app script's interpreter, for a test to call into.
    app: mlua::Lua,
}

impl LuaRig {
    fn new(src: &str) -> Self {
        let mut exts = Extensions::new();
        exts.push_as("kit", Box::new(LuaExtension::from_file(KIT).unwrap()))
            .unwrap();
        let app_ext = LuaExtension::from_source("app.lua", src).unwrap();
        let app = app_ext.lua().clone();
        exts.push_as("app", Box::new(app_ext)).unwrap();
        let mut core = Core::new();
        core.set_inspect(true);
        LuaRig { core, exts, warnings: Vec::new(), host: Vec::new(), app }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let mut ui = self.core.frame_with(VIEW, 1.0, &mut self.exts);
            ui.configure_root(NodeSpec::column().fill());
            ui.finish();
            let events = self.core.take_pending_events();
            let host = &mut self.host;
            self.exts.route(events, |ev| host.push(ev));
            self.warnings
                .extend(self.core.take_warnings().into_iter().map(|w| format!("{w:?}")));
        }
    }

    fn route(&mut self, events: Vec<UiEvent>) {
        let host = &mut self.host;
        self.exts.route(events, |ev| host.push(ev));
    }

    fn click(&mut self, x: f32, y: f32) {
        let events = click_at(&mut self.core, x, y);
        self.route(events);
    }

    /// A click as a window delivers one: the pointer arrives, then the
    /// press, then the release, a frame drawn between each.
    fn slow_click(&mut self, x: f32, y: f32) {
        use kui_core::{InputEvent, Vec2};
        let e = self.core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
        self.route(e);
        self.frames(1);
        let e = self.core.handle_input(InputEvent::mouse_down(1));
        self.route(e);
        self.frames(1);
        let e = self.core.handle_input(InputEvent::mouse_up());
        self.route(e);
        self.frames(1);
    }

    fn node(&mut self, pick: impl Fn(&kui_core::NodeInfo) -> bool) -> Option<kui_core::NodeInfo> {
        self.core.nodes().into_iter().find(|n| pick(n))
    }

    fn text(&mut self, s: &str) -> Option<kui_core::Rect> {
        self.node(|n| n.text.as_deref() == Some(s)).map(|n| n.rect)
    }

    fn named(&mut self, s: &str) -> kui_core::Rect {
        self.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some(s))
            .unwrap_or_else(|| panic!("no node named {s}"))
            .rect
    }
}

#[test]
fn a_select_opens_on_a_click_in_its_middle_and_closes_on_one_outside() {
    // In a toolbar's trailing group, as the samples place their skin
    // select: a click in the middle of the field opens it, a click on the
    // window outside closes it and is not swallowed.
    let mut r = LuaRig::new(
        r#"
        skin, clicks = "ledger", 0
        function on_event(ev)
          if ev.kind == "skin" then skin = ev.change.value end
          if ev.kind == "hit" then clicks = clicks + 1 end
        end
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/toolbar", {}, {
              text("Title"),
              trailing = {
                row { width = 120, component("kit/select", { label = "Skin", value = skin, on_change = { kind = "skin" },
                  options = { { value = "ledger", label = "Ledger" }, { value = "neutral", label = "Neutral" } } }) },
              },
            }),
            row { key = "elsewhere", label = "Elsewhere", role = "button", width = 200, height = 40, on_click = { kind = "hit" },
              text("clicks " .. clicks) },
          }
        end
        "#,
    );
    r.frames(2);
    let field = r.named("Skin: Ledger");
    r.slow_click(field.x + field.w / 2.0, field.y + field.h / 2.0);
    r.frames(2);
    assert!(r.text("Neutral").is_some(), "open after a click in the middle");
    let away = r.named("Elsewhere");
    r.slow_click(away.x + away.w / 2.0, away.y + away.h / 2.0);
    r.frames(2);
    assert!(r.text("Neutral").is_none(), "closed by the press outside");
    // The press that closes a menu is the menu's, as a native one's is;
    // the next reaches the window.
    r.slow_click(away.x + away.w / 2.0, away.y + away.h / 2.0);
    r.frames(1);
    assert!(r.text("clicks 1").is_some(), "the window is not blocked once it closed");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// Sixty one-line log rows in a 360×300 box, `wrap` as given.
fn log_app(wrap: bool) -> String {
    format!(
        r#"
        local cols = {{ time = {{}}, level = {{}}, message = {{}} }}
        for i = 1, 60 do
          cols.time[i], cols.level[i], cols.message[i] = string.format("12:%02d", i), "info", "line " .. i
        end
        local ch = channel(cols)
        function view(env)
          return column {{ width = "grow", height = "grow",
            column {{ key = "box", width = 360, height = 300,
              component("kit/log", {{ rows = ch, grow = true, wrap = {wrap} }}) }} }}
        end
        "#
    )
}

fn shows(r: &mut LuaRig, text: &str) -> bool {
    let bx = r.node(|n| n.label.as_deref() == Some("box")).expect("the box").rect;
    r.text(text).is_some_and(|t| t.y >= bx.y - 0.5 && t.y + t.h <= bx.y + bx.h + 0.5)
}

#[test]
fn a_log_scrolled_up_stays_where_the_reader_put_it() {
    // It followed its end on every frame it was asked, within a row of the
    // end: the first notch up was pulled back down.
    for wrap in [false, true] {
        let mut r = LuaRig::new(&log_app(wrap));
        r.frames(6);
        assert!(shows(&mut r, "line 60"), "at the end first (wrap {wrap})");
        let bx = r.node(|n| n.label.as_deref() == Some("box")).unwrap().rect;
        let (x, y) = (bx.x + bx.w / 2.0, bx.y + bx.h / 2.0);
        use kui_core::{InputEvent, Vec2};
        let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
        r.route(e);
        // A short move, as a glide's first frame is: it was within a row of the
        // end, which the old test still counted as the end.
        let e = r.core.handle_input(InputEvent::ScrollGesture { delta: Vec2::new(0.0, 10.0), begins: true });
        r.route(e);
        r.frames(4);
        assert!(!shows(&mut r, "line 60"), "the reader's scroll up stands (wrap {wrap})");
        assert!(r.text("LATEST").is_some(), "the jump back shows while it is away (wrap {wrap})");
        // Back down to the end: following again, no jump.
        let e = r.core.handle_input(InputEvent::ScrollGesture { delta: Vec2::new(0.0, -2000.0), begins: false });
        r.route(e);
        r.frames(4);
        assert!(shows(&mut r, "line 60"), "back at the end (wrap {wrap})");
        assert!(r.text("LATEST").is_none(), "following again (wrap {wrap})");
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_text_area_takes_a_click_anywhere_in_its_well() {
    // Only the line of text was the editor: a press in the empty well
    // under it, where a note's body is, focused nothing.
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = "grow",
            column { key = "note", width = 400,
              component("kit/text_area", { label = "Note", hide_label = true, value = "one line", rows = 8 }) } }
        end
        "#,
    );
    r.frames(2);
    let well = r.node(|n| n.label.as_deref() == Some("note")).unwrap().rect;
    r.slow_click(well.x + well.w / 2.0, well.y + well.h - 12.0);
    r.frames(1);
    let focused = r.core.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Note") && n.focused);
    assert!(focused, "the editor took the press in its well");
}

#[test]
fn a_switch_moves_its_knob_and_keeps_its_colour() {
    // The knob was the on_accent colour on, muted off, and jumped (a 0.12
    // ms transition): on and off read as two marks, not one that moved.
    let src = |on: bool| {
        format!(
            r#"
            function view(env)
              return column {{ width = "grow",
                component("kit/switch", {{ label = "Pinned", on = {on} }}) }}
            end
            "#
        )
    };
    let knob = |on: bool| {
        let mut r = LuaRig::new(&src(on));
        r.frames(2);
        let k = r.node(|n| n.label.as_deref() == Some("knob")).expect("the knob");
        (k.rect.x, k.bg)
    };
    let (off_x, off_bg) = knob(false);
    let (on_x, on_bg) = knob(true);
    assert!(on_x > off_x + 10.0, "on is to the right: {off_x} → {on_x}");
    assert_eq!(off_bg, on_bg, "the same knob in both states");
}

#[test]
fn a_toast_goes_by_itself_after_its_duration() {
    let mut r = LuaRig::new(
        r#"
        shown = true
        function on_event(ev) if ev.kind == "off" then shown = false end end
        function view(env)
          return column { width = "grow", height = "grow",
            shown and component("kit/toast", { message = "Saved.", duration = 1000, on_dismiss = { kind = "off" } }) or nil }
        end
        "#,
    );
    r.core.set_time(10.0);
    r.frames(2);
    assert!(r.text("Saved.").is_some(), "shown");
    r.core.set_time(10.5);
    r.frames(2);
    assert!(r.text("Saved.").is_some(), "still shown halfway");
    r.core.set_time(11.2);
    r.frames(3);
    assert!(r.text("Saved.").is_none(), "gone after its second");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn every_button_shades_under_the_pointer_and_the_press_and_stays_opaque() {
    // Primary and danger hovered to their own colour (no hover at all),
    // and every press was a translucent wash: under the primary button it
    // showed Ledger's hard shadow through the fill, so the shadow seemed
    // to jump into the button.
    for skin in ["ledger", "neutral", "phosphor"] {
        for variant in ["default", "primary", "danger", "success"] {
            let mut r = LuaRig::new(&format!(
                r#"
                function view(env)
                  return column {{ width = "grow", pad = 20,
                    component("kit/settings", {{ skin = "{skin}" }}),
                    component("kit/button", {{ label = "Go", variant = "{variant}" }}) }}
                end
                "#
            ));
            r.frames(2);
            let rest = r.named("Go");
            let (x, y) = (rest.x + rest.w / 2.0, rest.y + rest.h / 2.0);
            // The face where the button has one (Ledger's primary moves it
            // inside a still box that takes the pointer), the box otherwise.
            let bg = |r: &mut LuaRig| {
                let nodes = r.core.nodes();
                nodes
                    .iter()
                    .find(|n| n.label.as_deref() == Some("face"))
                    .or_else(|| nodes.iter().find(|n| n.rect.x == rest.x && n.rect.y == rest.y && n.rect.w == rest.w))
                    .map(|n| n.bg)
                    .expect("the button")
            };
            let rest_bg = bg(&mut r);
            use kui_core::{InputEvent, Vec2};
            let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
            r.route(e);
            r.frames(3);
            let hover_bg = bg(&mut r);
            let e = r.core.handle_input(InputEvent::mouse_down(1));
            r.route(e);
            r.frames(1);
            let press_bg = bg(&mut r);
            assert_ne!(rest_bg, hover_bg, "{skin} {variant}: a hover shows");
            assert_ne!(hover_bg, press_bg, "{skin} {variant}: a press shows");
            assert_eq!(press_bg.a, 1.0, "{skin} {variant}: the press is opaque: {press_bg:?}");
            assert_eq!(hover_bg.a, 1.0, "{skin} {variant}: the hover is opaque: {hover_bg:?}");
        }
    }
}

/// The solid quads of the last frame whose rect is `w`×`h` (± half a px).
fn quads_sized(core: &mut Core, w: f32, h: f32) -> Vec<kui_core::Quad> {
    kui_core::testing::solids(core)
        .into_iter()
        .filter(|q| (q.rect.w - w).abs() < 0.6 && (q.rect.h - h).abs() < 0.6)
        .collect()
}

#[test]
fn a_switch_knob_slides_across_and_settles_without_a_flash() {
    let mut r = LuaRig::new(
        r#"
        on = false
        function on_event(ev) if ev.kind == "pin" then on = ev.toggle.on end end
        function view(env)
          return column { width = "grow", pad = 20,
            component("kit/switch", { label = "Pinned", on = on, on_toggle = { kind = "pin" } }) }
        end
        "#,
    );
    r.core.set_time(1.0);
    r.frames(3);
    let sw = r.named("Pinned");
    let mut t = 1.0;
    r.click(sw.x + 8.0, sw.y + sw.h / 2.0);
    let mut xs = Vec::new();
    for _ in 0..16 {
        t += 0.016;
        r.core.set_time(t);
        r.frames(1);
        // The knob moved by layout (its alignment flips), which a
        // `transition` alone does not ease: it needs `slide`.
        let knobs = quads_sized(&mut r.core, 14.0, 14.0);
        xs.push(knobs.first().map(|q| q.rect.x).unwrap_or(f32::NAN));
    }
    let (first, last) = (xs[0], xs[xs.len() - 1]);
    assert!(last > first + 10.0, "it went across: {xs:?}");
    let between = xs.iter().filter(|x| **x > first + 1.0 && **x < last - 1.0).count();
    assert!(between >= 3, "it slid, frames in between: {xs:?}");
}

#[test]
fn tab_leaves_a_list_and_its_focus_shows() {
    // Relay's history: Tab went into the list and never out, with no ring.
    let mut r = LuaRig::new(
        r#"
        local ch = channel({ title = { "one", "two", "three" } })
        function view(env)
          return column { width = "grow", gap = 8,
            component("kit/button", { label = "Before" }),
            column { width = 300, height = 120, component("kit/list", { rows = ch, title = "title", grow = true }) },
            component("kit/button", { label = "After" }) }
        end
        "#,
    );
    r.frames(2);
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone()).or(Some("?".into()))
    };
    let press_tab = |r: &mut LuaRig| {
        use kui_core::testing::{key_press, tab};
        use kui_core::KeyCode;
        let e = r.core.handle_input(key_press(KeyCode::Tab));
        r.route(e);
        let e = tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
    };
    let mut seen = Vec::new();
    for _ in 0..4 {
        press_tab(&mut r);
        seen.push(focused(&mut r));
    }
    assert!(seen.iter().any(|f| f.as_deref() == Some("After")), "Tab got past the list: {seen:?}");
}

#[test]
fn ledger_primary_face_lifts_on_hover_and_sinks_on_press_over_a_still_shadow() {
    // The face and its hard shadow moved nowhere; the user wanted the
    // face to lift and sink while the shadow's far edge stays put.
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = "grow", pad = 20,
            component("kit/settings", { skin = "ledger" }),
            component("kit/button", { label = "Go", variant = "primary" }) }
        end
        "#,
    );
    r.core.set_time(1.0);
    r.frames(2);
    let face_and_shadow = |r: &mut LuaRig| {
        let face = r.node(|n| n.label.as_deref() == Some("face")).expect("the face").rect;
        let quads = kui_core::testing::quads_of(&mut r.core, kui_core::QuadKind::Shadow);
        let shadow = quads.first().map(|q| (q.rect.x + q.rect.w, q.rect.y + q.rect.h));
        // The face as drawn: its solid quad (the keyframe offset is paint).
        let drawn = kui_core::testing::solids(&mut r.core)
            .into_iter()
            .find(|q| (q.rect.w - face.w).abs() < 0.6 && (q.rect.h - face.h).abs() < 0.6)
            .map(|q| (q.rect.x, q.rect.y))
            .expect("the face's quad");
        (drawn, shadow)
    };
    let (rest, rest_shadow) = face_and_shadow(&mut r);
    let hit = r.named("Go");
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(hit.x + hit.w / 2.0, hit.y + hit.h / 2.0)));
    r.route(e);
    r.frames(2);
    let (hover, hover_shadow) = face_and_shadow(&mut r);
    r.core.set_time(1.1);
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    r.core.set_time(1.2);
    r.frames(1);
    r.core.set_time(1.3);
    r.frames(2);
    let (press, press_shadow) = face_and_shadow(&mut r);
    assert!(hover.0 < rest.0 && hover.1 < rest.1, "lifted: {rest:?} → {hover:?}");
    assert!(press.0 > rest.0 && press.1 > rest.1, "pushed in: {rest:?} → {press:?}");
    assert_eq!(rest_shadow, hover_shadow, "the shadow's far edge stays put under hover");
    assert!(press_shadow.is_none() || press_shadow == rest_shadow, "pressed, the shadow is under the face: {press_shadow:?}");
}

#[test]
fn a_slider_handle_stays_lit_while_held() {
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = 300, pad = 20,
            component("kit/slider", { label = "Zoom", value = 0.5, min = 0, max = 1 }) }
        end
        "#,
    );
    r.frames(2);
    let handle = |r: &mut LuaRig| r.node(|n| n.label.as_deref() == Some("handle")).expect("the handle");
    let rest = handle(&mut r).bg;
    let track = r.named("Zoom");
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(track.x + track.w * 0.25, track.y + track.h / 2.0)));
    r.route(e);
    r.frames(1);
    let over = handle(&mut r).bg;
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    r.frames(1);
    let held = handle(&mut r).bg;
    assert_ne!(rest, over, "lit under the pointer");
    assert_ne!(rest, held, "still lit while held");
}

#[test]
fn toasts_in_a_toaster_stack_without_overlapping() {
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/toaster", {}, {
              component("kit/toast", { key = "a", inline = true, message = "Saved.", tone = "ok", duration = 0 }),
              component("kit/toast", { key = "b", inline = true, message = "Could not reach the server.", tone = "danger", duration = 0 }),
            }) }
        end
        "#,
    );
    r.frames(2);
    let a = r.text("Saved.").expect("the first");
    let b = r.text("Could not reach the server.").expect("the second");
    assert!(b.y > a.y + a.h, "one above the other: {a:?} {b:?}");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_panel_title_rule_stays_inside_its_frame() {
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = "grow", pad = 20,
            component("kit/settings", { skin = "ledger" }),
            column { key = "outer", width = 300,
              component("kit/panel", { title = "Throughput" }, { text("body") }) } }
        end
        "#,
    );
    r.frames(2);
    let outer = r.node(|n| n.label.as_deref() == Some("outer")).unwrap().rect;
    // Every one-px-tall solid (the hairline under the title) starts past
    // the 2 px frame and ends before it.
    let lines: Vec<_> = kui_core::testing::solids(&mut r.core)
        .into_iter()
        .filter(|q| q.rect.h <= 1.5 && q.rect.w > 100.0)
        .collect();
    assert!(!lines.is_empty(), "the title's rule");
    for q in lines {
        assert!(q.rect.x >= outer.x + 1.5 && q.rect.x + q.rect.w <= outer.x + outer.w - 1.5, "inside the frame: {:?} in {outer:?}", q.rect);
    }
}

#[test]
fn a_log_republished_with_the_same_rows_keeps_its_place_and_one_that_drops_its_oldest_too() {
    // Watchtower's log republished its rows each second: every revision of
    // no new rows cleared the measured heights and the log jumped; and once
    // it was full, dropping the oldest moved what a reader scrolled up to.
    let mut r = LuaRig::new(
        r#"
        rows = {}
        for i = 1, 60 do rows[i] = i end
        local function publish()
          local cols = { time = {}, level = {}, message = {} }
          for j, i in ipairs(rows) do
            cols.time[j], cols.level[j] = string.format("t%03d", i), "info"
            cols.message[j] = i % 4 == 0 and ("line " .. i .. " is long enough to wrap onto a second row in a narrow log, and a third") or ("line " .. i)
          end
          return channel(cols)
        end
        ch = publish()
        function tick(drop)
          if drop then table.remove(rows, 1); rows[#rows + 1] = rows[#rows] + 1 end
          ch = publish()
        end
        function view(env)
          return column { width = "grow", height = "grow",
            column { key = "box", width = 360, height = 300,
              component("kit/log", { rows = ch, grow = true, wrap = true }) } }
        end
        function on_event(ev) end
        "#,
    );
    r.frames(6);
    let bx = r.node(|n| n.label.as_deref() == Some("box")).unwrap().rect;
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(bx.x + 100.0, bx.y + 100.0)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::ScrollGesture { delta: Vec2::new(0.0, 400.0), begins: true });
    r.route(e);
    r.frames(4);
    let first_seen = |r: &mut LuaRig| {
        let bx = r.node(|n| n.label.as_deref() == Some("box")).unwrap().rect;
        let mut seen: Vec<(f32, String)> = r
            .core
            .nodes()
            .into_iter()
            .filter_map(|n| n.text.filter(|t| t.starts_with("line ")).map(|t| (n.rect.y, t)))
            .filter(|(y, _)| *y >= bx.y)
            .collect();
        seen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        seen.first().cloned().expect("a row in view")
    };
    let before = first_seen(&mut r);
    for drop in [false, true, true] {
        r.app.globals().get::<mlua::Function>("tick").unwrap().call::<()>(drop).unwrap();
        r.frames(3);
        let now = first_seen(&mut r);
        assert_eq!(now.1, before.1, "the same row at the top (drop {drop})");
        assert!((now.0 - before.0).abs() < 1.0, "in the same place (drop {drop}): {before:?} → {now:?}");
    }
}

#[test]
fn a_dialog_shows_its_body() {
    // The body was a grow, scrolling column in a box with no height of its
    // own, and laid out at nothing: only the title and the buttons showed.
    let (core, w) = lua_app(
        r#"
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/dialog", { title = "Delete it?", on_dismiss = { kind = "x" } },
              { text("It goes for good."), actions = { component("kit/button", { label = "Delete" }) } }) }
        end
        "#,
        2,
    );
    assert!(w.is_empty(), "{w:#?}");
    let nodes = core.nodes();
    let text = nodes
        .iter()
        .find(|n| n.text.as_deref() == Some("It goes for good."))
        .expect("the body's text is built")
        .rect;
    // The text is laid out either way; the box it is clipped to was not.
    let body = nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("body"))
        .expect("the body box")
        .rect;
    assert!(
        body.h >= text.h,
        "the body is as tall as its text: {body:?} holding {text:?}"
    );
}

#[test]
fn a_named_popover_raises_no_warning() {
    let (_, w) = lua_app(
        r#"
        function view(env)
          return column { width = "grow",
            component("kit/popover", { label = "Scale", on_dismiss = { kind = "x" } }, { text("Instances") }) }
        end
        "#,
        2,
    );
    assert!(w.is_empty(), "{w:#?}");
}

#[test]
fn a_field_back_on_screen_does_not_replay_a_focus_asked_while_it_was_away() {
    // Field 1 shows, then field 2, the focus counter moves while 2 shows,
    // and 1 comes back: it took the moved number as a request in its first
    // frame back, when it had no node to focus ("no node is keyed edit").
    let (_, w) = lua_app(
        r#"
        local frame = 0
        function view(env)
          frame = frame + 1
          local which = (frame <= 2 or frame >= 6) and 1 or 2
          local focus = frame >= 5 and 1 or 0
          return column { width = "grow",
            column { key = "editor." .. which,
              component("kit/text_field", { label = "Title", value = "x", focus = focus }) } }
        end
        "#,
        8,
    );
    assert!(w.is_empty(), "{w:#?}");
}

#[test]
fn a_wrapped_log_follows_to_its_last_line() {
    // Pinned to the estimate's end, the wrapped log measured the rows it
    // showed, moved the estimate, and kept its window mid-log: the last
    // lines never showed and a blank band stood where they belonged.
    let (core, w) = lua_app(
        r#"
        local cols = { time = {}, level = {}, message = {} }
        for i = 1, 18 do
          cols.time[i], cols.level[i] = string.format("12:00:%02d", i), "info"
          cols.message[i] = i % 3 == 0
            and ("line " .. i .. " is a long one that goes on past the width of the log so that it wraps onto more rows than one")
            or ("line " .. i)
        end
        local ch = channel(cols)
        function view(env)
          return column { width = "grow", height = "grow",
            column { key = "box", width = 360, height = 300,
              component("kit/log", { rows = ch, grow = true, wrap = true }) } }
        end
        "#,
        8,
    );
    assert!(w.is_empty(), "{w:#?}");
    let nodes = core.nodes();
    let bx = nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("box"))
        .expect("the box")
        .rect;
    let last = nodes
        .iter()
        .find(|n| n.text.as_deref().is_some_and(|t| t.starts_with("line 18")))
        .expect("the last line is built")
        .rect;
    assert!(
        last.y >= bx.y - 0.5 && last.y + last.h <= bx.y + bx.h + 0.5,
        "the last line shows inside the log: {last:?} in {bx:?}"
    );
}

#[test]
fn a_notice_narrower_than_its_actions_puts_them_under_the_message() {
    let (core, w) = lua_app(
        r#"
        function view(env)
          return column { width = "grow",
            column { width = 250,
              component("kit/notice", { tone = "danger", message = "The disk is full, free some room and try again." },
                { component("kit/button", { label = "Retry" }) }) } }
        end
        "#,
        2,
    );
    assert!(w.is_empty(), "{w:#?}");
    let nodes = core.nodes();
    let msg = nodes
        .iter()
        .find(|n| n.text.as_deref().is_some_and(|t| t.starts_with("The disk")))
        .expect("the message")
        .rect;
    let retry = nodes
        .iter()
        .find(|n| n.text.as_deref() == Some("RETRY"))
        .expect("the action")
        .rect;
    assert!(
        retry.y >= msg.y + msg.h - 0.5,
        "the action is under the message: {retry:?} vs {msg:?}"
    );
}

#[test]
fn a_following_log_stays_at_its_end_the_frame_its_box_changes() {
    // A panel above the log grew: the log's view was replayed (nothing it
    // reads had changed yet), its pin to the end came a frame later, and
    // for that frame the last line sat below the box (or a band above
    // the foot when the box grew).
    for wrap in [false, true] {
        let mut r = LuaRig::new(&format!(
            r#"
            local cols = {{ time = {{}}, level = {{}}, message = {{}} }}
            for i = 1, 60 do
              cols.time[i], cols.level[i], cols.message[i] = string.format("12:%02d", i), "info", "line " .. i
            end
            local ch = channel(cols)
            above = 0
            function view(env)
              return column {{ width = "grow", height = "grow",
                column {{ key = "box", width = 360, height = 300,
                  row {{ width = "grow", height = above }},
                  column {{ width = "grow", height = "grow",
                    component("kit/log", {{ rows = ch, grow = true, wrap = {wrap} }}) }} }} }}
            end
            "#
        ));
        r.frames(6);
        for above in [80.0, 20.0, 120.0] {
            r.app.globals().set("above", above).unwrap();
            r.frames(1);
            let bx = r.node(|n| n.label.as_deref() == Some("box")).unwrap().rect;
            let last = r.text("line 60").expect("the last line is built");
            let foot = bx.y + bx.h;
            assert!(
                last.y + last.h <= foot + 0.5 && last.y + last.h >= foot - 30.0,
                "the last line at the foot the frame the box moved (wrap {wrap}, above {above}): {last:?} in {bx:?}"
            );
        }
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_line_charts_area_is_polygons_under_its_line_and_no_path() {
    // The area was one path through every point, rasterized again on every
    // tick: 3–4 ms a frame for 300 points. Polygons of eight points each
    // cost a quad apiece, and together they cover the plot's width.
    let (mut core, w) = lua_app(
        r#"
        local cols = { t = {}, a = {} }
        for i = 1, 300 do cols.t[i], cols.a[i] = i, 50 + 30 * math.sin(i / 9) end
        local ch = channel(cols)
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/line_chart", { data = ch, x = "t", area = true, height = 160,
              series = { { field = "a", name = "A" } } }) }
        end
        "#,
        4,
    );
    assert!(w.is_empty(), "{w:#?}");
    let nodes = core.nodes();
    assert!(nodes.iter().all(|n| n.kind != kui_core::NodeKind::Path), "no path");
    let polys: Vec<_> = nodes.iter().filter(|n| n.kind == kui_core::NodeKind::Polygon).collect();
    assert!(polys.len() > 10, "the area is cut into polygons: {}", polys.len());
    let line = nodes
        .iter()
        .filter(|n| n.kind == kui_core::NodeKind::Line)
        .max_by(|a, b| a.rect.w.total_cmp(&b.rect.w))
        .expect("the series' line")
        .rect;
    let left = polys.iter().map(|p| p.rect.x).fold(f32::MAX, f32::min);
    let right = polys.iter().map(|p| p.rect.x + p.rect.w).fold(f32::MIN, f32::max);
    assert!(
        (left - line.x).abs() <= 2.0 && (right - (line.x + line.w)).abs() <= 2.0,
        "the polygons span the line: {left}..{right} vs {line:?}"
    );
}

#[test]
fn a_charts_floats_are_cut_at_the_scroller_it_sits_in() {
    // The x labels and the hover bands are floats against their parent,
    // and a float escapes every clip unless it says `clip`: scrolled up,
    // the labels drew over the app's top bar and the bands took the
    // pointer there.
    let mut r = LuaRig::new(
        r#"
        local cols = { t = {}, a = {} }
        for i = 1, 60 do cols.t[i], cols.a[i] = i, 50 + 30 * math.sin(i / 9) end
        local ch = channel(cols)
        hovers, scrolled = 0, false
        function on_event(ev) if ev.kind == "hover" then hovers = hovers + 1 end end
        function view(env)
          if not scrolled then env.set_scroll("list", 0, 150); scrolled = true end
          return column { width = "grow", height = "grow",
            row { key = "top", label = "top", width = "grow", height = 60 },
            column { key = "list", width = "grow", height = 200, scroll_y = true,
              column { width = "grow", height = 100 },
              component("kit/line_chart", { data = ch, x = "t", height = 160, on_hover = { kind = "hover" },
                series = { { field = "a", name = "A" } } }),
              column { width = "grow", height = 300 } } }
        end
        "#,
    );
    r.frames(4);
    let top = r.node(|n| n.label.as_deref() == Some("top")).unwrap().rect;
    // Every x label above the scroller's top edge would be drawn on the bar.
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(top.x + top.w / 2.0, top.y + top.h - 10.0)));
    r.route(e);
    r.frames(2);
    let over_bar: i64 = r.app.globals().get("hovers").unwrap();
    assert_eq!(over_bar, 0, "a band past the scroller's edge takes no pointer");
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(top.x + top.w / 2.0, top.y + top.h + 30.0)));
    r.route(e);
    r.frames(2);
    let inside: i64 = r.app.globals().get("hovers").unwrap();
    assert!(inside > 0, "inside the scroller the bands still hover");
}
