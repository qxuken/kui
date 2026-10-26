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
    put("skeleton", kit::Skeleton::new().lines(3).place(ui));
    put("shader", kit::Shader::new().height(40.0).label("Noise").place(ui));
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
    // tick. Polygons of eight points each cost a quad apiece, and together
    // they cover the plot's width; a single path, linear since C96, still
    // rasterizes the plot-wide mask each tick and benched slower.
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
        "the area spans the line: {left}..{right} vs {line:?}"
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

/// Every focus-ring colour a skin draws, light and dark, as quads carry it.
fn focus_colours() -> Vec<kui_core::Color> {
    [0x2f6fedffu32, 0x7fa8ffff, 0x5b8cffff, 0x0f7a35ff, 0x39ff6aff]
        .iter()
        .map(|&c| kui_core::Color::hex(c))
        .collect()
}

fn focus_quads(core: &mut Core) -> usize {
    let ring = focus_colours();
    kui_core::testing::solids(core)
        .iter()
        .filter(|q| ring.iter().any(|c| q.color == *c || (q.border_w > 0.0 && q.border_color == *c)))
        .count()
}

#[test]
fn focusing_a_field_moves_nothing() {
    // Ledger's underline went from a hair to the region rule on focus, a
    // pixel taller, and the text area's edge thickened inward: the rows
    // under a field, and the text in a text area, moved when it took the
    // keyboard.
    for skin in ["ledger", "phosphor", "neutral"] {
        let mut r = LuaRig::new(&format!(
            r#"
            function view(env)
              return column {{ width = "grow", height = "grow", gap = 4,
                component("kit/settings", {{ skin = "{skin}" }}),
                component("kit/text_field", {{ value = "", label = "Name", hide_label = true }}),
                row {{ key = "after_field", label = "after_field", width = 10, height = 10 }},
                component("kit/text_area", {{ value = "first line", label = "Notes", hide_label = true }}),
                row {{ key = "after_area", label = "after_area", width = 10, height = 10 }},
              }}
            end
            "#
        ));
        r.frames(3);
        let at = |r: &mut LuaRig| {
            let area = r
                .core
                .nodes()
                .into_iter()
                .filter(|n| n.kind == kui_core::NodeKind::Edit)
                .nth(1)
                .expect("the text area's editor")
                .key;
            // Its text, by the first glyph drawn inside its box.
            let bx = r.node(|n| n.key == area).unwrap().rect;
            let first = kui_core::testing::quads_of(&mut r.core, kui_core::QuadKind::GlyphMask)
                .into_iter()
                .filter(|q| q.rect.y >= bx.y && q.rect.y < bx.y + bx.h)
                .map(|q| (q.rect.x, q.rect.y))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .expect("a glyph of its text");
            (
                r.node(|n| n.label.as_deref() == Some("after_field")).unwrap().rect.y,
                r.node(|n| n.label.as_deref() == Some("after_area")).unwrap().rect.y,
                first,
            )
        };
        let rest = at(&mut r);
        kui_core::testing::tab(&mut r.core, false);
        r.frames(2);
        assert_eq!(at(&mut r), rest, "the field focused ({skin})");
        kui_core::testing::tab(&mut r.core, false);
        r.frames(2);
        assert_eq!(at(&mut r), rest, "the text area focused ({skin})");
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_fields_error_takes_no_room_of_its_own() {
    // The error line under a field made the toolbar holding it taller, and
    // its buttons, centred on the taller row, left the field's line.
    let mut r = LuaRig::new(
        r#"
        err = nil
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/toolbar", {}, {
              component("kit/text_field", { value = "", placeholder = "What needs doing?", error = err }),
              component("kit/button", { label = "Add", variant = "primary" }),
            }),
            row { key = "below", label = "below", width = 10, height = 10 },
          }
        end
        "#,
    );
    r.frames(3);
    let below = r.node(|n| n.label.as_deref() == Some("below")).unwrap().rect.y;
    let add = r.named("Add");
    r.app.globals().set("err", "Name the task first").unwrap();
    r.frames(3);
    assert!(r.text("Name the task first").is_some(), "the message shows");
    assert_eq!(r.node(|n| n.label.as_deref() == Some("below")).unwrap().rect.y, below, "the toolbar keeps its height");
    assert_eq!(r.named("Add").y, add.y, "the button keeps its line");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_checkbox_ring_hugs_its_box_and_shows_only_from_the_keyboard() {
    // The core's ring went round the checkbox's whole row, its full height,
    // far from the 14 px box; and a click showed rings that only the
    // keyboard should.
    let mut r = LuaRig::new(
        r#"
        on = false
        function on_event(ev) if ev.kind == "t" then on = ev.toggle.checked end end
        function view(env)
          return column { width = "grow", height = "grow", pad = 20,
            component("kit/checkbox", { label = "Pinned", hide_label = true, checked = on, on_toggle = { kind = "t" } }) }
        end
        "#,
    );
    r.frames(3);
    let row = r.named("Pinned");
    r.slow_click(row.x + row.w / 2.0, row.y + row.h / 2.0);
    r.frames(2);
    assert_eq!(focus_quads(&mut r.core), 0, "a click focuses without a ring");
    kui_core::testing::tab(&mut r.core, true);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(2);
    let ring = r
        .node(|n| n.float && (n.rect.w - 22.0).abs() < 0.5 && (n.rect.h - 22.0).abs() < 0.5)
        .expect("the ring round the box")
        .rect;
    let bx = r
        .node(|n| (n.rect.w - 14.0).abs() < 0.5 && (n.rect.h - 14.0).abs() < 0.5 && n.rect.x >= row.x - 0.5)
        .expect("the box")
        .rect;
    assert!((bx.x - ring.x - 4.0).abs() < 0.5 && (bx.y - ring.y - 4.0).abs() < 0.5, "2 px out: {ring:?} round {bx:?}");
    assert!(focus_quads(&mut r.core) > 0, "the keyboard shows it");
}

#[test]
fn a_click_on_a_list_row_draws_no_ring_and_the_keyboard_does() {
    let mut r = LuaRig::new(
        r#"
        local ch = channel({ name = { "one", "two", "three" } })
        sel = nil
        function on_event(ev) if ev.kind == "sel" then sel = ev.select.row end end
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/button", { label = "Before" }),
            column { width = 300, height = 200,
              component("kit/list", { rows = ch, title = "name", selected = sel, grow = true, on_select = { kind = "sel" } }) } }
        end
        "#,
    );
    r.frames(3);
    let two = r.text("two").unwrap();
    r.slow_click(two.x + 2.0, two.y + two.h / 2.0);
    r.frames(2);
    assert_eq!(focus_quads(&mut r.core), 0, "no ring round the list after a click");
    // From nothing, Tab to the button and Tab again into the list.
    r.core.set_focus(None);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(1);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(2);
    assert!(focus_quads(&mut r.core) > 0, "Tab into it shows the ring");
}

#[test]
fn a_slider_handle_is_grabbed_and_drags() {
    // The handle showed the arrow while the track showed the hand, and a
    // press on it was its own, not the slider's: it could not be dragged.
    // The track is an open hand now, the handle with it, a closed one
    // while held, and a drag that starts on the handle moves the value.
    let mut r = LuaRig::new(
        r#"
        v = 0.5
        function on_event(ev) if ev.kind == "zoom" then v = ev.change.value end end
        function view(env)
          return column { width = 300, height = "grow", pad = 20,
            component("kit/slider", { value = v, min = 0, max = 1, label = "Zoom", on_change = { kind = "zoom" } }) }
        end
        "#,
    );
    r.frames(3);
    let handle = r
        .node(|n| (n.rect.w - 12.0).abs() < 0.5 && (n.rect.h - 12.0).abs() < 0.5)
        .expect("the handle")
        .rect;
    let (x, y) = (handle.x + 6.0, handle.y + 6.0);
    use kui_core::{InputEvent, Vec2, cursor::CursorShape};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    r.route(e);
    r.frames(2);
    assert_eq!(r.core.cursor_shape(), CursorShape::Grab, "an open hand over the handle");
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    r.frames(2);
    assert_eq!(r.core.cursor_shape(), CursorShape::Grabbing, "a closed one while it is held");
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(x + 60.0, y)));
    r.route(e);
    r.frames(2);
    assert_eq!(r.core.cursor_shape(), CursorShape::Grabbing, "still closed off the handle");
    let e = r.core.handle_input(InputEvent::mouse_up());
    r.route(e);
    r.frames(2);
    let v: f64 = r.app.globals().get("v").unwrap();
    assert!(v > 0.6, "the drag from the handle moved the value: {v}");
}

#[test]
fn a_toast_with_no_tone_is_info_and_the_last_one_slides_out() {
    // A toast with no tone was grey, its edge the rule colour; and the
    // last toast in a toaster vanished, since the host took the toaster
    // away with it and a node going with its parent skips its exit.
    let mut r = LuaRig::new(
        r#"
        shown = true
        function view(env)
          return column { width = "grow", height = "grow",
            component("kit/settings", { skin = "ledger" }),
            shown and component("kit/toaster", {}, {
              component("kit/toast", { message = "Saved.", inline = true, duration = 0 }) }) or nil }
        end
        "#,
    );
    r.core.set_time(1.0);
    r.frames(3);
    r.core.set_time(2.0);
    r.frames(2);
    let edge = r
        .node(|n| (n.rect.w - 4.0).abs() < 0.5 && n.rect.h > 20.0)
        .expect("the tone's edge");
    assert!(
        focus_colours().contains(&edge.bg),
        "the edge is the skin's info blue: {:?}",
        edge.bg
    );
    let glyphs = |r: &mut LuaRig| kui_core::testing::quads_of(&mut r.core, kui_core::QuadKind::GlyphMask).len();
    let with = glyphs(&mut r);
    r.app.globals().set("shown", false).unwrap();
    r.frames(1);
    r.core.set_time(2.06);
    r.frames(1);
    assert!(glyphs(&mut r) >= with, "still drawn while it slides out");
    r.core.set_time(2.5);
    r.frames(2);
    assert!(glyphs(&mut r) < with, "gone once it has");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_new_notes_title_takes_the_keyboard_and_tab_goes_on_to_its_body() {
    // Field Notes: New note opened the note, and the keyboard stayed on
    // the button.
    let mut r = LuaRig::new(
        r#"
        id = nil
        function on_event(ev) if ev.kind == "new" then id = (id or 0) + 1 end end
        function view(env)
          return column { width = "grow", height = "grow", gap = 8,
            component("kit/button", { label = "New note", on_press = { kind = "new" } }),
            id and column { key = "editor." .. id, width = "grow", gap = 8,
              component("kit/text_field", { key = "title", label = "Title", hide_label = true, value = "Untitled", autofocus = true }),
              component("kit/text_area", { key = "body", label = "Note", hide_label = true, value = "" }),
            } or nil }
        end
        "#,
    );
    r.frames(2);
    let b = r.named("New note");
    r.slow_click(b.x + b.w / 2.0, b.y + b.h / 2.0);
    r.frames(3);
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone())
    };
    assert_eq!(focused(&mut r).as_deref(), Some("Title"), "the title has the keyboard");
    kui_core::testing::tab(&mut r.core, false);
    r.frames(2);
    assert_eq!(focused(&mut r).as_deref(), Some("Note"), "Tab goes on to the body");
}

/// A raw key press as a window sends one: the key down, then its editing
/// key, routed.
fn press_key(r: &mut LuaRig, code: kui_core::KeyCode) {
    let kp = kui_core::KeyPress::new(code, kui_core::KeyMods::default());
    let edit = kp.edit_event();
    let e = r.core.handle_input(kui_core::InputEvent::KeyDown(kp));
    r.route(e);
    if let Some(ev) = edit {
        let e = r.core.handle_input(ev);
        r.route(e);
    }
    r.frames(1);
}

#[test]
fn a_tables_rows_are_one_stop_the_arrows_walk() {
    // Tab stopped on the table and the arrows did nothing visible; and a
    // table nobody selects in was a stop that did nothing at all.
    for selectable in [true, false] {
        let mut r = LuaRig::new(&format!(
            r#"
            local ch = channel({{ name = {{ "one", "two", "three", "four" }} }})
            sel = nil
            function on_event(ev) if ev.kind == "sel" then sel = ev.select.row end end
            function view(env)
              return column {{ width = "grow", height = "grow",
                component("kit/button", {{ label = "Before" }}),
                column {{ width = 300, height = 200,
                  component("kit/table", {{ rows = ch, columns = {{ {{ field = "name", title = "Name" }} }},
                    selected = sel, grow = true, on_select = {selectable} and {{ kind = "sel" }} or nil }}) }},
                component("kit/button", {{ label = "After" }}) }}
            end
            "#
        ));
        r.frames(2);
        let focused = |r: &mut LuaRig| -> Option<String> {
            r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone())
        };
        r.core.set_focus(None);
        let mut stops = Vec::new();
        for _ in 0..4 {
            kui_core::testing::tab(&mut r.core, false);
            r.frames(1);
            stops.push(focused(&mut r));
        }
        // The rows are a list composite (ADR 0070): one stop, entered on
        // the first row, the selection following the keyboard.
        let rows = ["one", "two", "three", "four"];
        let into = stops.iter().position(|s| s.as_deref().is_some_and(|s| rows.contains(&s)));
        if !selectable {
            assert!(into.is_none(), "a table nobody selects in is no stop: {stops:?}");
            continue;
        }
        assert_eq!(stops.iter().filter(|s| s.as_deref().is_some_and(|s| rows.contains(&s))).count(), 1,
            "the rows are one stop: {stops:?}");
        r.core.set_focus(None);
        r.app.globals().set("sel", mlua::Value::Nil).unwrap();
        r.frames(1);
        for _ in 0..=into.unwrap() {
            kui_core::testing::tab(&mut r.core, false);
            r.frames(1);
        }
        assert_eq!(focused(&mut r).as_deref(), Some("one"));
        use kui_core::KeyCode;
        press_key(&mut r, KeyCode::Down);
        r.frames(1);
        let sel: Option<i64> = r.app.globals().get("sel").unwrap();
        assert_eq!(sel, Some(1), "Down selects the next row");
        press_key(&mut r, KeyCode::End);
        r.frames(1);
        let sel: Option<i64> = r.app.globals().get("sel").unwrap();
        assert_eq!(sel, Some(3), "End the last");
        press_key(&mut r, KeyCode::Up);
        r.frames(1);
        let sel: Option<i64> = r.app.globals().get("sel").unwrap();
        assert_eq!(sel, Some(2), "Up the one before");
        press_key(&mut r, KeyCode::Home);
        r.frames(1);
        let sel: Option<i64> = r.app.globals().get("sel").unwrap();
        assert_eq!(sel, Some(0), "Home the first");
    }
}

#[test]
fn a_bar_charts_bars_are_one_stop_the_arrows_walk_and_look_hovered() {
    // Every bar was a Tab stop, ringed apart from the look a hovered bar
    // has and with no tooltip; and the value column left a gap wider than
    // a bar between "6%" and the first bar.
    let src = |selects: bool| {
        format!(
            r#"
            local ch = channel({{ name = {{ "a", "b", "c" }}, rate = {{ 5, 2.9, 1.3 }} }})
            picked = nil
            function on_event(ev) if ev.kind == "pick" then picked = ev.select.row end end
            function view(env)
              return column {{ width = 400, height = "grow",
                component("kit/button", {{ label = "Before" }}),
                component("kit/bar_chart", {{ data = ch, label = "name", value = "rate", unit = "%", height = 160,
                  on_select = {selects} and {{ kind = "pick" }} or nil }}),
                component("kit/button", {{ label = "After" }}) }}
            end
            "#
        )
    };
    let mut r = LuaRig::new(&src(true));
    r.frames(2);
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone())
    };
    let mut stops = Vec::new();
    for _ in 0..3 {
        kui_core::testing::tab(&mut r.core, false);
        r.frames(1);
        stops.push(focused(&mut r));
    }
    assert_eq!(stops[1].as_deref(), Some("a: 5%"), "the bars are one stop, the first bar: {stops:?}");
    assert_eq!(stops[2].as_deref(), Some("After"), "and the next stop is past them: {stops:?}");
    kui_core::testing::tab(&mut r.core, true);
    r.frames(1);
    use kui_core::{EditKey, InputEvent, Mods};
    r.core.handle_input(InputEvent::Key(EditKey::Right, Mods::NONE));
    r.frames(2);
    assert_eq!(focused(&mut r).as_deref(), Some("b: 2.9%"), "Right walks to the next bar");
    let items: Vec<_> = r.core.nodes().into_iter().filter(|n| n.role == Some(kui_core::Role::ListItem)).collect();
    let (a, bar) = (items[0].clone(), items[1].clone());
    assert_ne!(bar.bg, a.bg, "the bar the keyboard is on looks hovered");
    assert!(r.text("b: 2.9%").is_some(), "its tooltip shows");
    let e = r.core.handle_input(InputEvent::Key(EditKey::Enter, Mods::NONE));
    r.route(e);
    r.frames(1);
    let picked: Option<i64> = r.app.globals().get("picked").unwrap();
    assert_eq!(picked, Some(1), "Enter picks it");
    // The value column hugs its widest label.
    let six = r.text("6%").expect("the top value label");
    assert!(a.rect.x - (six.x + six.w) <= 10.0, "the first bar sits by the labels: {six:?} {:?}", a.rect);

    let mut r = LuaRig::new(&src(false));
    r.frames(2);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(1);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("After"), "bars nobody picks are no stop");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_waterfalls_spans_are_one_stop_or_none() {
    // Every span bar was a Tab stop, and one that did nothing where the
    // host picks no spans (Relay's trace).
    let src = |picks: bool| {
        format!(
            r#"
            local ch = channel({{ name = {{ "load", "parse", "paint" }}, start = {{ 0, 2, 30 }}, len = {{ 60, 20, 10 }},
              depth = {{ 0, 1, 1 }}, group = {{ 0, 1, 2 }}, target = {{ "a", "b", "c" }} }})
            picked = nil
            function on_event(ev) if ev.kind == "pick" then picked = ev.select.row end end
            function view(env)
              return column {{ width = 600, height = "grow",
                component("kit/waterfall", {{ bars = ch, on_select = {picks} and {{ kind = "pick" }} or nil }}),
                component("kit/button", {{ label = "After" }}) }}
            end
            "#
        )
    };
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone())
    };
    for picks in [true, false] {
        let mut r = LuaRig::new(&src(picks));
        r.frames(3);
        let mut stops = Vec::new();
        for _ in 0..6 {
            kui_core::testing::tab(&mut r.core, false);
            r.frames(1);
            stops.push(focused(&mut r));
        }
        let spans = stops.iter().filter(|s| matches!(s.as_deref(), Some("load" | "parse" | "paint"))).count();
        if picks {
            // The three tools, the spans once, After, round to the first.
            assert_eq!(spans, 1, "the spans are one stop: {stops:?}");
            let at = stops.iter().position(|s| matches!(s.as_deref(), Some("load" | "parse" | "paint"))).unwrap();
            r.core.set_focus(None);
            for _ in 0..=at {
                kui_core::testing::tab(&mut r.core, false);
                r.frames(1);
            }
            let first = focused(&mut r);
            use kui_core::{EditKey, InputEvent, Mods};
            r.core.handle_input(InputEvent::Key(EditKey::Down, Mods::NONE));
            r.frames(1);
            assert_ne!(focused(&mut r), first, "an arrow walks to the next span");
            let e = r.core.handle_input(InputEvent::Key(EditKey::Enter, Mods::NONE));
            r.route(e);
            r.frames(1);
            let picked: Option<i64> = r.app.globals().get("picked").unwrap();
            assert!(picked.is_some(), "Enter picks it");
        } else {
            assert_eq!(spans, 0, "spans nobody picks are no stop: {stops:?}");
        }
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_tables_ring_and_selection_are_one_cursor() {
    // Watchtower: a click on a row focused that row (a row with a click
    // and no role was a control of its own), the arrows moved the
    // selection through the table above it, and the ring stayed on the
    // row first clicked.
    let mut r = LuaRig::new(
        r#"
        local ch = channel({ name = { "one", "two", "three", "four" } })
        sel = nil
        function on_event(ev) if ev.kind == "sel" then sel = ev.select.row end end
        function view(env)
          return column { width = "grow", height = "grow",
            column { width = 300, height = 200,
              component("kit/table", { rows = ch, columns = { { field = "name", title = "Name" } },
                selected = sel, grow = true, on_select = { kind = "sel" } }) } }
        end
        "#,
    );
    r.frames(2);
    let two = r.text("two").unwrap();
    r.slow_click(two.x + 2.0, two.y + two.h / 2.0);
    r.frames(1);
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone())
    };
    // A row is the list composite's item (ADR 0070): the click focuses it,
    // and the arrows move that focus, the selection with it.
    assert_eq!(focused(&mut r).as_deref(), Some("two"), "the click focuses the row");
    press_key(&mut r, kui_core::KeyCode::Down);
    press_key(&mut r, kui_core::KeyCode::Down);
    r.frames(1);
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(3), "the arrows moved the selection");
    let ring = focus_colours();
    let rings: Vec<kui_core::Quad> = kui_core::testing::solids(&mut r.core)
        .into_iter()
        .filter(|q| q.border_w > 0.0 && ring.contains(&q.border_color))
        .collect();
    let four = r.text("four").unwrap();
    assert_eq!(rings.len(), 1, "one ring: {rings:?}");
    let q = rings[0].rect;
    assert!(q.y <= four.y && q.y + q.h >= four.y + four.h, "on the selected row: {q:?} vs {four:?}");
}

#[test]
fn rings_are_in_the_skins_focus_colour() {
    // The core's ring is the theme's blue in every skin; Kit's controls
    // draw their own in the skin's focus colour, the core's turned off.
    for (skin, colours) in [("phosphor", [0x0f7a35ffu32, 0x39ff6aff]), ("neutral", [0x2f6fedff, 0x5b8cffff])] {
        let mut r = LuaRig::new(&format!(
            r#"
            function view(env)
              return column {{ width = "grow", height = "grow", pad = 20, gap = 8,
                component("kit/settings", {{ skin = "{skin}" }}),
                component("kit/button", {{ label = "Go" }}),
                component("kit/segmented", {{ value = "a", options = {{ {{ value = "a", label = "A" }}, {{ value = "b", label = "B" }} }} }}),
                component("kit/tabs", {{ value = "x", tabs = {{ {{ value = "x", label = "X" }}, {{ value = "y", label = "Y" }} }} }}, {{}}),
                component("kit/slider", {{ value = 0.5, label = "Zoom" }}),
                component("kit/section", {{ title = "More" }}, {{}}),
              }}
            end
            "#
        ));
        r.frames(2);
        let skin_ring: Vec<kui_core::Color> = colours.iter().map(|&c| kui_core::Color::hex(c)).collect();
        let theme_ring = [kui_core::Theme::light().focus_ring, kui_core::Theme::dark().focus_ring];
        for stop in 0..5 {
            kui_core::testing::tab(&mut r.core, false);
            r.frames(2);
            let quads = kui_core::testing::solids(&mut r.core);
            let ours = quads.iter().filter(|q| q.border_w > 0.0 && skin_ring.contains(&q.border_color)).count();
            let theirs = quads.iter().filter(|q| q.border_w > 0.0 && theme_ring.contains(&q.border_color)).count();
            let who = r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone());
            assert!(ours >= 1, "{skin}, stop {stop} ({who:?}): a ring in the skin's colour");
            assert_eq!(theirs, 0, "{skin}, stop {stop} ({who:?}): no ring in the theme's");
        }
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_slider_lands_on_the_step_nearest_the_pointer_and_its_handle_on_the_value() {
    // The handle was laid out after the fill, half a handle and more past
    // the value's place, while the core maps the pointer along the track:
    // dragging the handle landed a step past the one under it.
    let mut r = LuaRig::new(
        r#"
        v = 5
        function on_event(ev) if ev.kind == "v" then v = ev.change.value end end
        function view(env)
          return column { width = 400, height = "grow", pad = 20,
            component("kit/slider", { value = v, min = 0, max = 10, step = 1, label = "Level", on_change = { kind = "v" } }) }
        end
        "#,
    );
    r.frames(3);
    let track = r.node(|n| n.role == Some(kui_core::Role::Slider)).expect("the track").rect;
    let (start, len) = (track.x + 6.0, track.w - 12.0);
    let handle = r
        .node(|n| (n.rect.w - 12.0).abs() < 0.5 && (n.rect.h - 12.0).abs() < 0.5)
        .expect("the handle")
        .rect;
    let centre = handle.x + 6.0;
    assert!((centre - (start + 0.5 * len)).abs() <= 1.0, "the handle sits on 5: {centre} vs {}", start + 0.5 * len);
    let y = track.y + track.h / 2.0;
    use kui_core::{InputEvent, Vec2};
    let mut drag_to = |r: &mut LuaRig, at: f32| -> f64 {
        let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(centre, y)));
        r.route(e);
        r.frames(1);
        let e = r.core.handle_input(InputEvent::mouse_down(1));
        r.route(e);
        r.frames(1);
        let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(start + at / 10.0 * len, y)));
        r.route(e);
        r.frames(1);
        let e = r.core.handle_input(InputEvent::mouse_up());
        r.route(e);
        r.frames(2);
        r.app.globals().get("v").unwrap()
    };
    assert_eq!(drag_to(&mut r, 5.4), 5.0, "5.4 is nearest 5");
    assert_eq!(drag_to(&mut r, 5.6), 6.0, "5.6 is nearest 6");
    assert_eq!(drag_to(&mut r, 2.45), 2.0, "2.45 is nearest 2");
}

#[test]
fn an_under_way_bar_crosses_its_whole_track() {
    // It slid from -80 px to 400 px whatever the track's width: on a wide
    // track it turned back a third of the way along.
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = 900, height = "grow", pad = 20,
            component("kit/progress", { label = "Under way" }) }
        end
        "#,
    );
    r.core.set_time(1.0);
    r.frames(3);
    let track = r.node(|n| n.role == Some(kui_core::Role::Slider)).expect("the track").rect;
    let mut right = f32::MIN;
    let mut left = f32::MAX;
    for k in 0..60 {
        r.core.set_time(1.0 + k as f64 * 0.05);
        r.frames(1);
        for q in kui_core::testing::solids(&mut r.core) {
            let bar = (q.rect.h - 4.0).abs() < 0.5 && (q.rect.w - 0.3 * track.w).abs() < 2.0;
            if bar {
                right = right.max(q.rect.x + q.rect.w);
                left = left.min(q.rect.x);
            }
        }
    }
    assert!(right >= track.x + track.w - 2.0, "it reaches the track's end: {right} vs {:?}", track);
    assert!(left <= track.x + 2.0, "and starts from its start: {left} vs {:?}", track);
}

#[test]
fn a_fields_choose_button_asks_for_a_folder_and_fills_the_field() {
    // Tidewater's sync folder could only be typed.
    let mut r = LuaRig::new(
        r#"
        folder = "/Users/me/Tidewater"
        function on_event(ev) if ev.kind == "folder" then folder = ev.change.text end end
        function view(env)
          return column { width = 500, height = "grow", pad = 20,
            component("kit/text_field", { label = "Sync folder", value = folder, mono = true, browse = "folder",
              on_change = { kind = "folder" } }) }
        end
        "#,
    );
    r.frames(2);
    let b = r.named("Choose… sync folder");
    r.slow_click(b.x + b.w / 2.0, b.y + b.h / 2.0);
    r.frames(2);
    let asks = r.core.take_file_requests();
    assert_eq!(asks.len(), 1, "one dialog asked for");
    assert_eq!(asks[0].mode, kui_core::FileDialogMode::Folder);
    assert_eq!(asks[0].directory.as_deref(), Some("/Users/me/Tidewater"),
        "it starts in the field's folder");
    let e = r.core.handle_input(kui_core::InputEvent::Files(vec!["/Volumes/Data/Sync".into()]));
    r.route(e);
    r.frames(2);
    let folder: String = r.app.globals().get("folder").unwrap();
    assert_eq!(folder, "/Volumes/Data/Sync", "the path picked is the field's");
    // Cancelled: nothing changes.
    r.slow_click(b.x + b.w / 2.0, b.y + b.h / 2.0);
    r.frames(2);
    assert_eq!(r.core.take_file_requests().len(), 1);
    let e = r.core.handle_input(kui_core::InputEvent::Files(vec![]));
    r.route(e);
    r.frames(2);
    let folder: String = r.app.globals().get("folder").unwrap();
    assert_eq!(folder, "/Volumes/Data/Sync", "a cancel changes nothing");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_narrow_row_shrinks_its_field_and_keeps_its_button_inside_the_panel() {
    // Tidewater made narrow: the sync folder's field kept its 440 px, and
    // its Choose… button stood out past the panel's frame, its label cut.
    for w in [760.0, 600.0, 520.0] {
        let mut r = LuaRig::new(&format!(
            r#"
            function view(env)
              return column {{ width = {w}, height = "grow",
                component("kit/panel", {{ key = "folder", title = "Folder" }}, {{
                  row {{ width = "grow", gap = 24, pad = 10,
                    column {{ width = 250, text("Sync folder") }},
                    column {{ width = "grow", min_width = 0,
                      component("kit/text_field", {{ label = "Sync folder", hide_label = true, value = "/Users/me/Pictures",
                        mono = true, width = 440, browse = "folder" }}),
                      component("kit/select", {{ label = "Mode", value = "a", width = 300,
                        options = {{ {{ value = "a", label = "A rather long choice" }} }} }}) }} }} }}) }}
            end
            "#
        ));
        r.frames(3);
        let panel = r.node(|n| n.role == Some(kui_core::Role::Group) && n.label.as_deref() == Some("Folder"))
            .map(|n| n.rect)
            .unwrap_or_else(|| r.named("Folder"));
        let choose = r.named("Choose… sync folder");
        let mode = r.named("Mode: A rather long choice");
        let inside = |b: kui_core::Rect| b.x >= panel.x - 0.5 && b.x + b.w <= panel.x + panel.w - 1.5;
        assert!(inside(choose), "at {w}: the button inside the panel: {choose:?} in {panel:?}");
        assert!(inside(mode), "at {w}: the select inside the panel: {mode:?} in {panel:?}");
        let label = r
            .node(|n| n.text.as_deref().is_some_and(|t| t.to_lowercase().starts_with("choose")))
            .expect("the button's label")
            .rect;
        assert!(label.x + label.w <= choose.x + choose.w, "at {w}: its whole label");
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_long_list_steps_past_its_window_and_types_ahead() {
    // The list was a key sink stepping the selection by index; it is a
    // windowed list composite now (ADR 0070): End reaches a row the
    // window never built, and a search the built rows cannot answer is
    // answered from the channel.
    let mut r = LuaRig::new(
        r#"
        local names = {}
        for i = 1, 1000 do names[i] = string.format("item %04d", i - 1) end
        names[701] = "zebra"
        local ch = channel({ name = names })
        sel = nil
        function on_event(ev) if ev.kind == "sel" then sel = ev.select.row end end
        function view(env)
          return column { width = 300, height = 300,
            component("kit/list", { rows = ch, title = "name", label = "Items", selected = sel, grow = true,
              on_select = { kind = "sel" } }) }
        end
        "#,
    );
    r.frames(2);
    let e = kui_core::testing::tab(&mut r.core, false);
    r.route(e);
    r.frames(2);
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(0), "Tab lands on the first row and selects it");
    press_key(&mut r, kui_core::KeyCode::End);
    r.frames(4);
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(999), "End reaches the last row, past the window");
    // A search among the rows the window built is the core's own.
    let e = r.core.handle_input(kui_core::InputEvent::Text("item 0995".into()));
    r.route(e);
    r.frames(2);
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(995), "a type-ahead lands on a built row");
    // One past the window asks Kit, which answers from the channel in its
    // draw, the core taking an answer given while a view builds and
    // applying it when the frame ends (backlog C104).
    let e = r.core.handle_input(kui_core::InputEvent::Text("z".into()));
    r.route(e);
    r.frames(4);
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(700), "answered in a draw");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_table_of_cells_walks_its_cells() {
    // `cells = true`: a table of rows of focusable cells (ADR 0070), the
    // header a row of column headers; Right moves along the row, Down to
    // the row below keeping the column, and landing on a row selects it.
    let mut r = LuaRig::new(
        r#"
        local ch = channel({ name = { "auth", "orders", "search" }, state = { "up", "down", "up" } })
        sel = nil
        function on_event(ev) if ev.kind == "sel" then sel = ev.select.row end end
        function view(env)
          return column { width = 400, height = 300,
            component("kit/table", { rows = ch, cells = true, selected = sel, grow = true, on_select = { kind = "sel" },
              columns = { { field = "name", title = "Name" }, { field = "state", title = "State" } } }) }
        end
        "#,
    );
    r.frames(2);
    let tree = r.core.access_tree();
    assert!(tree.nodes.iter().any(|n| n.role == kui_core::Role::Table), "a table");
    assert!(tree.nodes.iter().any(|n| n.role == kui_core::Role::ColumnHeader), "with column headers");
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone())
    };
    let auth = r.text("auth").unwrap();
    r.slow_click(auth.x + 2.0, auth.y + auth.h / 2.0);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("auth"));
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(0), "a click on a cell selects its row, as arrowing onto it does");
    press_key(&mut r, kui_core::KeyCode::Right);
    assert_eq!(focused(&mut r).as_deref(), Some("up"), "Right along the row");
    press_key(&mut r, kui_core::KeyCode::Down);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("down"), "Down keeps the column");
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(sel, Some(1), "and selects the row it lands on");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_text_area_types_tab_unless_it_moves_focus() {
    // C99: a multiline editor types Tab by default; `tab_moves_focus` is a
    // note's, whose Tab moves on as a field's does.
    for moves in [false, true] {
        let mut r = LuaRig::new(&format!(
            r#"
            body = ""
            function on_event(ev) if ev.kind == "b" then body = ev.change.text end end
            function view(env)
              return column {{ width = 400, height = "grow",
                component("kit/text_area", {{ label = "Body", value = body, autofocus = true,
                  tab_moves_focus = {moves}, on_change = {{ kind = "b" }} }}),
                component("kit/button", {{ label = "After" }}) }}
            end
            "#
        ));
        r.frames(3);
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(2);
        let body: String = r.app.globals().get("body").unwrap();
        let on = r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone());
        if moves {
            assert_eq!(on.as_deref(), Some("After"), "Tab moved on");
            assert_eq!(body, "");
        } else {
            assert_eq!(on.as_deref(), Some("Body"), "focus stays");
            assert_eq!(body, "    ", "Tab typed");
        }
    }
}

#[test]
fn under_reduced_motion_a_switch_knob_jumps() {
    // Kit's eased props jump when the user asked for less motion, by a
    // `reduced_motion` rule on every node Kit animates (ADR 0069).
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
    let mut system = r.core.env.system;
    system.motion = kui_core::MotionPref::Reduced;
    r.core.set_system(system);
    r.core.set_time(1.0);
    r.frames(3);
    let sw = r.named("Pinned");
    let before = quads_sized(&mut r.core, 14.0, 14.0)[0].rect.x;
    r.click(sw.x + 8.0, sw.y + sw.h / 2.0);
    let mut xs = Vec::new();
    let mut t = 1.0;
    for _ in 0..6 {
        t += 0.016;
        r.core.set_time(t);
        r.frames(1);
        xs.push(quads_sized(&mut r.core, 14.0, 14.0)[0].rect.x);
    }
    let last = xs[xs.len() - 1];
    assert!(last > before + 10.0, "it went across: {xs:?}");
    assert!(xs.iter().all(|x| (*x - last).abs() < 0.5), "in one frame, no steps between: {xs:?}");
}

#[test]
fn a_line_chart_breaks_its_line_at_a_gap_and_keeps_its_window() {
    // The line comes from the channel's reduction (C95) now: a value that
    // is no number breaks it, and the window's start is a binary search.
    let (core, w) = lua_app(
        r#"
        local t, a = {}, {}
        for i = 1, 200 do t[i] = i; a[i] = 50 + 30 * math.sin(i / 9) end
        a[120] = 0 / 0
        local ch = channel({ t = t, a = a })
        function view(env)
          return column { width = 600, height = "grow",
            component("kit/line_chart", { data = ch, x = "t", window = 150, height = 160,
              series = { { field = "a", name = "A" } } }) }
        end
        "#,
        4,
    );
    assert!(w.is_empty(), "{w:#?}");
    let runs: Vec<kui_core::Rect> = core
        .nodes()
        .iter()
        .filter(|n| n.kind == kui_core::NodeKind::Line && n.rect.w > 40.0 && n.rect.h > 20.0)
        .map(|n| n.rect)
        .collect();
    assert_eq!(runs.len(), 2, "two runs either side of the gap: {runs:?}");
    let (a, b) = (runs[0], runs[1]);
    assert!(a.x + a.w <= b.x + 1.0 || b.x + b.w <= a.x + 1.0, "side by side, not overlapping: {runs:?}");
}

/// The x labels of a line chart `width` px wide over x 0..=100: each text
/// node ending in ` s`, left to right.
fn x_labels(width: u32) -> Vec<(kui_core::Rect, String)> {
    let (core, w) = lua_app(
        &format!(
            r#"
            local t, a = {{}}, {{}}
            for i = 0, 100 do t[#t + 1] = i; a[#a + 1] = 50 + 30 * math.sin(i / 9) end
            local ch = channel({{ t = t, a = a }})
            function view(env)
              return column {{ width = {width}, height = "grow",
                component("kit/line_chart", {{ data = ch, x = "t", x_unit = "s", height = 160,
                  series = {{ {{ field = "a", name = "A" }} }} }}) }}
            end
            "#
        ),
        4,
    );
    assert!(w.is_empty(), "{w:#?}");
    let mut labels: Vec<(kui_core::Rect, String)> = core
        .nodes()
        .into_iter()
        .filter_map(|n| n.text.filter(|t| t.ends_with(" s")).map(|t| (n.rect, t)))
        .collect();
    labels.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    labels
}

#[test]
fn a_line_charts_x_labels_are_spaced_by_their_measured_widths() {
    // The step was a guess — a tick every 90 px, every label 80 px wide —
    // so labels went long before they touched. Each is measured now, and
    // ticks come as close as two neighbours' half widths and a gap.
    let wide = x_labels(1100);
    let texts: Vec<&str> = wide.iter().map(|(_, t)| t.as_str()).collect();
    assert!(wide.len() > 11, "more ticks than one every 90 px: {texts:?}");
    assert!(texts.iter().all(|t| !t.contains('.')), "whole steps read whole: {texts:?}");
    for pair in wide.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        assert!(a.0.x + a.0.w + 4.0 <= b.0.x, "{a:?} and {b:?} do not touch");
    }
    // Narrow, it steps out to labels that still fit.
    let narrow = x_labels(260);
    let texts: Vec<&str> = narrow.iter().map(|(_, t)| t.as_str()).collect();
    assert!(narrow.len() >= 3, "{texts:?}");
    for pair in narrow.windows(2) {
        assert!(pair[0].0.x + pair[0].0.w + 4.0 <= pair[1].0.x, "{texts:?}");
    }
}

/// A line chart's y labels: the text nodes that read as plain numbers
/// (with ` ms` after them, for a unit).
fn y_labels(core: &Core) -> Vec<(kui_core::Rect, f64)> {
    core.nodes()
        .into_iter()
        .filter_map(|n| {
            let t = n.text?;
            let v: f64 = t.trim_end_matches(" ms").trim_end_matches('k').parse().ok()?;
            Some((n.rect, v))
        })
        .collect()
}

#[test]
fn a_line_charts_gutter_is_as_wide_as_its_widest_label() {
    // The y labels took 44 px, or 56 with a unit, whatever they said.
    for (top, unit, old) in [(9.0, "nil", 44.0), (12000.0, "\"ms\"", 56.0)] {
        let (core, w) = lua_app(
            &format!(
                r#"
                local t, a = {{}}, {{}}
                for i = 1, 50 do t[i] = i; a[i] = {top} * i / 50 end
                local ch = channel({{ t = t, a = a }})
                function view(env)
                  return column {{ width = 600, height = "grow",
                    component("kit/line_chart", {{ data = ch, x = "t", x_unit = "s", y_unit = {unit}, height = 160,
                      series = {{ {{ field = "a", name = "A" }} }} }}) }}
                end
                "#
            ),
            4,
        );
        assert!(w.is_empty(), "{w:#?}");
        let ys: Vec<_> = y_labels(&core).into_iter().filter(|(r, _)| r.x < 100.0).collect();
        assert!(ys.len() >= 3, "{ys:?}");
        let right = ys.iter().map(|(r, _)| r.x + r.w).fold(f32::MIN, f32::max);
        let widest = ys.iter().map(|(r, _)| r.w).fold(f32::MIN, f32::max);
        assert!(right - widest <= 4.0, "the gutter hugs its widest label ({top}): {right} vs {widest}");
        assert!(right < old, "narrower than the old fixed gutter ({top}): {right}");
        for (r, _) in &ys {
            assert!((r.x + r.w - right).abs() < 1.0, "right-aligned: {ys:?}");
        }
    }
}

#[test]
fn a_line_chart_glides_to_a_new_range_and_jumps_under_reduced_motion() {
    for reduced in [false, true] {
        let mut r = LuaRig::new(
            r#"
            local function make(top)
              local t, a = {}, {}
              for i = 1, 50 do t[i] = i; a[i] = top * i / 50 end
              return channel({ t = t, a = a })
            end
            local small, big = make(10), make(100)
            ch = small
            function grow() ch = big end
            function view(env)
              return column { width = 600, height = "grow",
                component("kit/line_chart", { data = ch, x = "t", x_unit = "s", height = 160,
                  series = { { field = "a", name = "A" } } }) }
            end
            "#,
        );
        if reduced {
            let mut system = r.core.env.system;
            system.motion = kui_core::MotionPref::Reduced;
            r.core.set_system(system);
        }
        r.core.set_time(1.0);
        r.frames(3);
        let top = |r: &mut LuaRig| y_labels(&r.core).iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
        assert_eq!(top(&mut r), 10.0);
        r.app.globals().get::<mlua::Function>("grow").unwrap().call::<()>(()).unwrap();
        r.core.set_time(1.05);
        r.frames(1);
        r.core.set_time(1.1);
        r.frames(1);
        let mid = top(&mut r);
        if reduced {
            assert_eq!(mid, 100.0, "under reduced motion the range jumps");
        } else {
            assert!(mid > 10.0 && mid < 100.0, "on its way: {mid}");
            assert!(r.core.animating(), "a frame is asked for while it glides");
        }
        r.core.set_time(1.6);
        r.frames(2);
        assert_eq!(top(&mut r), 100.0, "there in the end (reduced {reduced})");
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

/// Kit's shaders, as Kit registers them itself (C111) and a host may too.
const SHIMMER: &str = include_str!("../../../addons/kit/shaders/shimmer.wgsl");
const NOISE: &str = include_str!("../../../addons/kit/shaders/noise.wgsl");

fn reduce_motion(core: &mut Core) {
    let mut system = core.env.system;
    system.motion = kui_core::MotionPref::Reduced;
    core.set_system(system);
}

#[test]
fn kit_carries_its_shaders_as_the_files_say() {
    // build.lua embeds shaders/*.wgsl into the Lua every host loads.
    let data = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../addons/kit/lua/data.lua")).unwrap();
    for (name, src) in [("shimmer", SHIMMER), ("noise", NOISE)] {
        assert!(data.contains(src), "lua/data.lua carries shaders/{name}.wgsl as it is");
        assert!(Core::new().add_fragment(src).is_some(), "shaders/{name}.wgsl compiles");
    }
}

#[test]
fn a_skeleton_sweeps_kits_own_shimmer_and_holds_still_under_reduced_motion() {
    for reduced in [false, true] {
        let mut r = LuaRig::new(
            r#"
            function view(env)
              return column { width = 400, pad = 10, gap = 10,
                component("kit/skeleton", { lines = 3, label = "Loading the list" }),
                component("kit/skeleton", { shape = "circle", width = 40 }),
                component("kit/skeleton", { shape = "block", height = 60 }) }
            end
            "#,
        );
        if reduced {
            reduce_motion(&mut r.core);
        }
        r.core.set_time(1.0);
        r.frames(3);
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
        r.named("Loading the list");
        // Every bar a fragment of Kit's shimmer, registered by Kit: three
        // text lines, the last 60% of the others, a circle and a block.
        let frags: Vec<kui_core::Rect> = r
            .core
            .nodes()
            .into_iter()
            .filter(|n| n.kind == kui_core::NodeKind::Fragment)
            .map(|n| n.rect)
            .collect();
        assert_eq!(frags.len(), 5, "{frags:?}");
        let lines: Vec<&kui_core::Rect> = frags.iter().filter(|b| b.h == 10.0).collect();
        assert_eq!(lines.len(), 3, "{frags:?}");
        assert!((lines[2].w - lines[0].w * 0.6).abs() < 1.0, "the last line is shorter: {lines:?}");
        assert!(frags.iter().any(|b| b.w == 40.0 && b.h == 40.0), "a circle 40 across");
        assert_eq!(r.core.animating(), !reduced, "it sweeps unless motion is reduced");
    }
}

#[test]
fn a_shader_box_draws_a_preset_a_source_or_the_hosts_fragment_and_is_a_well_without_one() {
    for reduced in [false, true] {
        for preset in [kit::ShaderPreset::Shimmer, kit::ShaderPreset::Noise] {
            let mut r = rig();
            if reduced {
                reduce_motion(&mut r.core);
            }
            for _ in 0..2 {
                r.frame(|ui| {
                    kit::Settings::new().skin(kit::Skin::Neutral).place(ui);
                    kit::Shader::new().preset(preset).height(50.0).label("A preset").place(ui);
                });
            }
            let w = r.core.take_warnings();
            assert!(w.is_empty(), "{w:#?}");
            let frag = r.core.nodes().into_iter().find(|n| n.kind == kui_core::NodeKind::Fragment).expect("the fragment");
            assert_eq!(frag.rect.h, 50.0);
            r.centre("A preset");
            assert_eq!(r.core.animating(), !reduced, "a preset moves unless motion is reduced ({preset:?})");
        }
    }
    // WGSL handed over is registered by Kit; the host's own handle is drawn.
    let mut r = rig();
    let id = r.core.add_fragment(NOISE).unwrap();
    r.frame(|ui| {
        kit::Shader::new().source(SHIMMER).height(20.0).place(ui);
        kit::Shader::new().id(id.to_ffi() as i64).params(vec![0.0; 12]).height(21.0).place(ui);
    });
    assert!(r.core.take_warnings().is_empty());
    let frags = r.core.nodes().into_iter().filter(|n| n.kind == kui_core::NodeKind::Fragment).count();
    assert_eq!(frags, 2);
    assert!(!r.core.animating(), "a caller's shader moves only when it says animate");
    // Source that does not compile is said, and the box is the well.
    let mut r = rig();
    r.frame(|ui| {
        kit::Shader::new().source("fn fragment(").height(30.0).place(ui);
        kit::Shader::new().height(31.0).place(ui);
    });
    let w = r.core.take_warnings();
    assert!(w.iter().any(|w| w.code == "fragment-rejected"), "{w:#?}");
    assert!(r.core.nodes().iter().all(|n| n.kind != kui_core::NodeKind::Fragment), "no shader, no fragment");
    assert!(r.core.nodes().iter().any(|n| n.rect.h == 30.0) && r.core.nodes().iter().any(|n| n.rect.h == 31.0), "the wells keep their place");
}

#[test]
fn a_select_opens_on_the_press_with_its_option_over_the_field_and_a_drag_picks() {
    // C108: the press opens it, the chosen option lands over the field,
    // and a release over another option picks it, as a native pop-up does.
    let mut r = LuaRig::new(
        r#"
        mode = "two"
        function on_event(ev) if ev.kind == "mode" then mode = ev.change.value end end
        function view(env)
          return column { width = "grow", height = "grow", pad = 200,
            row { width = 200, component("kit/select", { label = "Mode", value = mode, on_change = { kind = "mode" },
              options = { { value = "one", label = "One" }, { value = "two", label = "Two" }, { value = "three", label = "Three" } } }) } }
        end
        "#,
    );
    r.frames(2);
    let field = r.named("Mode: Two");
    use kui_core::{InputEvent, Vec2};
    let (fx, fy) = (field.x + field.w / 2.0, field.y + field.h / 2.0);
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(fx, fy)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    r.frames(2);
    let two = r.named("Two");
    let mid = two.y + two.h / 2.0;
    assert!(mid > field.y && mid < field.y + field.h, "the current option sits over the field: {two:?} vs {field:?}");
    let three = r.named("Three");
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(fx, three.y + three.h / 2.0)));
    r.route(e);
    r.frames(1);
    let e = r.core.handle_input(InputEvent::mouse_up());
    r.route(e);
    r.frames(2);
    let mode: String = r.app.globals().get("mode").unwrap();
    assert_eq!(mode, "three", "released over Three, it picks Three");
    assert!(r.text("One").is_none(), "and closes");
    // A quick click opens it and leaves it open, its option over the field.
    let field = r.named("Mode: Three");
    r.slow_click(field.x + field.w / 2.0, field.y + field.h / 2.0);
    r.frames(2);
    assert!(r.text("One").is_some(), "open after a quick click");
    let mode: String = r.app.globals().get("mode").unwrap();
    assert_eq!(mode, "three", "and nothing picked");
    // C109: an accelerator closes it and goes on to the window.
    let e = kui_core::testing::key_down(&mut r.core, kui_core::KeyCode::Char('c'), kui_core::KeyMods::NONE.with_primary());
    r.route(e);
    r.frames(2);
    assert!(r.text("One").is_none(), "⌘C closed the menu");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}


#[test]
fn a_code_blocks_copy_puts_its_text_on_the_clipboard_and_says_so() {
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = 500, pad = 10,
            component("kit/code", { text = "local a = 1\nlocal b = 2\nreturn a + b", copy = true,
              line_numbers = true, label = "sum.lua", max_lines = 10 }) }
        end
        "#,
    );
    r.core.set_time(1.0);
    r.frames(2);
    assert!(r.text("1\n2\n3").is_some(), "the lines' numbers in their gutter");
    r.named("sum.lua");
    let b = r.named("Copy the code");
    r.click(b.x + b.w / 2.0, b.y + b.h / 2.0);
    r.frames(2);
    let copied: Vec<_> = r
        .core
        .take_menu_actions()
        .into_iter()
        .filter_map(|a| match a {
            kui_core::MenuAction::SetClipboard { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(copied, vec!["local a = 1\nlocal b = 2\nreturn a + b".to_string()]);
    r.named("Copied");
    r.core.set_time(3.0);
    r.frames(2);
    r.named("Copy the code");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn an_accent_brings_what_the_skin_paints_in_its_accent_and_an_appearance_holds() {
    // Ledger paints its selection in its accent: a brand's blue accent left
    // it yellow, with black on the blue primary.
    let mut r = rig();
    let mut seen = Vec::new();
    for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
        r.frame(|ui| {
            let roles = Value::map([("accent", Value::Int(0x1d4ed8ff))]);
            kit::Settings::new().appearance(appearance).roles(roles).place(ui);
            let t = |ui: &mut Ui<'_>, n: &str| ui.token_color(n).expect(n);
            seen.push((t(ui, "kitAccent"), t(ui, "kitSel"), t(ui, "kitOnAccent"), t(ui, "kitSelFg"), t(ui, "kitBg"), t(ui, "kitFg")));
        });
    }
    let (accent, sel, on, sel_fg, bg, _) = seen[0];
    assert_eq!(sel, accent, "the selection follows the accent");
    assert!(on.r > 0.9 && on.g > 0.9, "light on the dark blue, not the ink: {on:?}");
    assert_eq!(sel_fg, on, "and on the selection");
    let (_, _, _, _, dark_bg, dark_fg) = seen[1];
    assert_ne!(dark_bg, bg, "dark held: the dark half's page");
    assert_ne!(dark_fg, seen[0].5, "and its text");
}

/// WCAG's contrast between two opaque colours.
fn contrast(a: kui_core::Color, b: kui_core::Color) -> f32 {
    let lum = |c: kui_core::Color| {
        let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    };
    let (x, y) = (lum(a), lum(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

/// `top`, with its alpha, laid over the opaque `under`.
fn over(under: kui_core::Color, top: kui_core::Color) -> kui_core::Color {
    let m = |u: f32, t: f32| u + (t - u) * top.a;
    kui_core::Color { r: m(under.r, top.r), g: m(under.g, top.g), b: m(under.b, top.b), a: 1.0 }
}

#[test]
fn an_accent_derives_a_palette_whose_text_reads_everywhere() {
    // A yellow accent left Ledger's info notice yellow on paper and
    // Phosphor's text and knobs green. Every skin, both halves, four
    // accents: each derived text on what it sits on reads at 4.5:1, a chart
    // line at 3:1, and the palette leans toward the accent.
    let accents = [0xffd400ffi64, 0x7fe9ffff, 0x1e2a5aff, 0xdc2626ff];
    for skin in [kit::Skin::Ledger, kit::Skin::Neutral, kit::Skin::Phosphor] {
        for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
            let mut plain = std::collections::HashMap::new();
            let mut r = rig();
            r.frame(|ui| {
                kit::Settings::new().skin(skin).appearance(appearance).place(ui);
            });
            r.frame(|ui| {
                for n in ["kitBg", "kitPanel", "kitFg", "kitChosen", "kitAccent"] {
                    plain.insert(n, ui.token_color(n).unwrap());
                }
            });
            for accent in accents {
                let mut got = std::collections::HashMap::new();
                let names = [
                    "kitAccent", "kitOnAccent", "kitSel", "kitSelFg", "kitBg", "kitSide", "kitPanel", "kitSunken",
                    "kitRaised", "kitFg", "kitMuted", "kitFaint", "kitFocus", "kitOk", "kitWarn", "kitDanger",
                    "kitAccentText", "kitChosen", "kitChosenFg", "kitGroup1", "kitSeries1",
                ];
                r.frame(|ui| {
                    let roles = Value::map([("accent", Value::Int(accent))]);
                    kit::Settings::new().skin(skin).appearance(appearance).roles(roles).place(ui);
                });
                r.frame(|ui| {
                    for n in names {
                        got.insert(n, ui.token_color(n).unwrap_or_else(|| panic!("{n}")));
                    }
                });
                let at = |n: &str| got[n];
                let case = format!("{skin:?} {appearance:?} {accent:08x}");
                let grounds = ["kitBg", "kitSide", "kitPanel", "kitSunken", "kitRaised"];
                for text in ["kitFg", "kitMuted", "kitFaint"] {
                    for g in grounds {
                        let k = contrast(at(text), at(g));
                        assert!(k >= 4.49, "{case}: {text} on {g} reads at {k:.2}");
                    }
                }
                for text in ["kitFocus", "kitOk", "kitWarn", "kitDanger", "kitAccentText"] {
                    for g in ["kitPanel", "kitSunken", "kitBg"] {
                        let k = contrast(at(text), at(g));
                        assert!(k >= 4.49, "{case}: {text} on {g} reads at {k:.2}");
                    }
                }
                let pairs = [("kitOnAccent", "kitAccent"), ("kitChosenFg", "kitChosen"), ("kitFg", "kitGroup1")];
                for (text, fill) in pairs {
                    let k = contrast(at(text), at(fill));
                    assert!(k >= 4.49, "{case}: {text} on {fill} reads at {k:.2}");
                }
                let sel = over(at("kitPanel"), at("kitSel"));
                let k = contrast(at("kitSelFg"), sel);
                assert!(k >= 4.49, "{case}: the selection's text reads at {k:.2}");
                let k = contrast(at("kitSeries1"), at("kitPanel"));
                assert!(k >= 2.99, "{case}: the first series reads at {k:.2}");
                // Derived, not left as it was: the ground leans toward the
                // accent and the chosen segment is the accent.
                if at("kitAccent") != plain["kitAccent"] {
                    assert_ne!(at("kitBg"), plain["kitBg"], "{case}: the ground is tinted");
                }
                assert_eq!(at("kitChosen"), at("kitAccent"), "{case}: a view switch's chosen segment");
            }
        }
    }
}

#[test]
fn the_keyboard_opens_a_select_on_its_current_option() {
    // The platform's convention (backlog C116), each faked with
    // `Core::set_platform`, which `env.platform` reads. On macOS Up, Down
    // (Alt or not), F4, Space and Enter on a closed, focused select open
    // its menu with focus on the option it shows. On Windows and Linux
    // Alt+Up, Alt+Down, F4, Space and Enter open it, and a bare Up or Down
    // steps the value to the previous or next enabled option, clamped,
    // without opening. Escape closes an open one and gives focus back.
    use kui_core::{KeyCode, KeyMods, Platform};
    const SRC: &str = r#"
        value = value or "two"
        changed = nil
        function on_event(ev) if ev.kind == "mode" then changed = ev.change.value; value = ev.change.value end end
        function view(env)
          return column { width = "grow", height = "grow", pad = 100,
            row { width = 200, component("kit/select", { label = "Mode", value = value, on_change = { kind = "mode" },
              options = { { value = "zero", label = "Zero", disabled = true }, { value = "one", label = "One" },
                { value = "two", label = "Two" }, { value = "skip", label = "Skip", disabled = true },
                { value = "three", label = "Three" } } }) } }
        end
    "#;
    let rig = |platform: Platform, value: &str| {
        let mut r = LuaRig::new(SRC);
        r.core.set_platform(platform);
        r.app.globals().set("value", value).unwrap();
        r.frames(2);
        let e = r.core.handle_input(kui_core::testing::key_press(KeyCode::Tab));
        r.route(e);
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
        r
    };
    let focused = |r: &mut LuaRig| r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone());
    // As a window delivers it: the press, and the space's text.
    let press = |r: &mut LuaRig, code: KeyCode, mods: KeyMods| {
        let e = r.core.press(kui_core::KeyPress::new(code, mods));
        r.route(e);
        if code == KeyCode::Space {
            let e = r.core.handle_input(kui_core::InputEvent::Text(" ".into()));
            r.route(e);
        }
        r.frames(3);
    };
    let alt = KeyMods::NONE.with_alt();
    let mac: &[(KeyCode, KeyMods)] = &[
        (KeyCode::Down, KeyMods::NONE),
        (KeyCode::Up, KeyMods::NONE),
        (KeyCode::Down, alt),
        (KeyCode::F(4), KeyMods::NONE),
        (KeyCode::Space, KeyMods::NONE),
        (KeyCode::Enter, KeyMods::NONE),
    ];
    let others: &[(KeyCode, KeyMods)] = &[
        (KeyCode::Down, alt),
        (KeyCode::Up, alt),
        (KeyCode::F(4), KeyMods::NONE),
        (KeyCode::Space, KeyMods::NONE),
        (KeyCode::Enter, KeyMods::NONE),
    ];
    for (platform, opening) in [(Platform::MacOs, mac), (Platform::Windows, others), (Platform::Linux, others)] {
        for &(code, mods) in opening {
            let mut r = rig(platform, "two");
            assert_eq!(focused(&mut r).as_deref(), Some("Mode: Two"), "Tab reached the field");
            press(&mut r, code, mods);
            assert!(r.text("Three").is_some(), "{platform:?} {code:?} {mods:?} opened the menu");
            assert_eq!(focused(&mut r).as_deref(), Some("Two"), "{platform:?} {code:?}: focus on the current option");
            press_key(&mut r, KeyCode::Escape);
            r.frames(2);
            assert!(r.text("Three").is_none(), "Escape closed it");
            assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
        }
    }
    // Off macOS a bare arrow steps over the disabled options, and stops at
    // the ends: from Two, Down is Three (past Skip) and Up is One; from
    // Three, Down is nothing; from One, Up is nothing (Zero is disabled).
    for platform in [Platform::Windows, Platform::Linux] {
        for (from, code, to) in [
            ("two", KeyCode::Down, Some("three")),
            ("two", KeyCode::Up, Some("one")),
            ("three", KeyCode::Down, None),
            ("one", KeyCode::Up, None),
        ] {
            let mut r = rig(platform, from);
            press(&mut r, code, KeyMods::NONE);
            let changed: Option<String> = r.app.globals().get("changed").unwrap();
            assert_eq!(changed.as_deref(), to, "{platform:?}: {code:?} from {from}");
            assert!(r.text("Skip").is_none(), "{platform:?}: {code:?} from {from} left it closed");
            if let Some(to) = to {
                let label = format!("Mode: {}{}", to[..1].to_uppercase(), &to[1..]);
                assert_eq!(focused(&mut r).as_deref(), Some(label.as_str()), "the field shows the step, focused");
            }
            assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
        }
        // And again from where it stepped to: Three, then nowhere.
        let mut r = rig(platform, "one");
        press(&mut r, KeyCode::Down, KeyMods::NONE);
        press(&mut r, KeyCode::Down, KeyMods::NONE);
        let value: String = r.app.globals().get("value").unwrap();
        assert_eq!(value, "three", "{platform:?}: two steps down from One");
    }
}

#[test]
fn tab_and_shift_tab_leave_a_select() {
    // The select's field is a key sink for its arrows, Space and Enter;
    // Tab and Shift-Tab still walk the ring, out of it both ways.
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = "grow", height = "grow", pad = 40, gap = 8,
            component("kit/button", { label = "Before" }),
            row { width = 200, component("kit/select", { label = "Mode", value = "two", on_change = { kind = "mode" },
              options = { { value = "one", label = "One" }, { value = "two", label = "Two" } } }) },
            component("kit/button", { label = "After" }) }
        end
        "#,
    );
    r.frames(2);
    let focused = |r: &mut LuaRig| r.core.access_tree().nodes.iter().find(|n| n.focused).and_then(|n| n.name.clone());
    let tab = |r: &mut LuaRig, back: bool| {
        use kui_core::testing::tab;
        let mods = if back { kui_core::KeyMods::NONE.with_shift() } else { kui_core::KeyMods::NONE };
        let kp = kui_core::InputEvent::KeyDown(kui_core::KeyPress::new(kui_core::KeyCode::Tab, mods));
        let e = r.core.handle_input(kp);
        r.route(e);
        let e = tab(&mut r.core, back);
        r.route(e);
        r.frames(1);
    };
    tab(&mut r, false);
    tab(&mut r, false);
    assert_eq!(focused(&mut r).as_deref(), Some("Mode: Two"), "on the select");
    tab(&mut r, false);
    assert_eq!(focused(&mut r).as_deref(), Some("After"), "Tab leaves it forward");
    tab(&mut r, true);
    assert_eq!(focused(&mut r).as_deref(), Some("Mode: Two"), "back on it");
    tab(&mut r, true);
    assert_eq!(focused(&mut r).as_deref(), Some("Before"), "Shift-Tab leaves it back");
    assert!(r.text("One").is_none(), "and the menu never opened");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_sliders_handle_is_cut_at_the_scroller_it_sits_in() {
    // Scrolled up past the scroller's top, the slider's handle drew over
    // the app's bar: a float escapes every clip unless it says `clip`.
    let mut r = LuaRig::new(
        r#"
        scrolled = false
        function view(env)
          if not scrolled then env.set_scroll("pane", 0, 30); scrolled = true end
          return column { width = "grow", height = "grow",
            row { key = "bar", label = "bar", width = "grow", height = 80 },
            column { key = "pane", label = "pane", width = 300, height = 120, scroll_y = true, pad = 10,
              component("kit/slider", { label = "Rule", value = 0.5, min = 0, max = 1 }),
              column { width = "grow", height = 400 } } }
        end
        "#,
    );
    r.frames(4);
    let pane = r.named("pane");
    let handle = quads_sized(&mut r.core, 12.0, 12.0);
    assert!(!handle.is_empty(), "the handle is drawn");
    let (dl, _) = r.core.output();
    for q in &handle {
        let clip = dl.clips[q.clip as usize].rect;
        assert!(clip.y >= pane.y - 0.5, "the handle is cut at the pane's top: clip {clip:?}, pane {pane:?}");
    }
}
