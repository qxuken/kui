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
            kit::Markdown::new("## Notes\n\n- **one** and `two`").place(ui);
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
    put(
        "checklist",
        kit::Checklist::new(vec![
            kit::Check::new("Milk", true),
            kit::Check::new("Eggs", false),
        ])
        .removable(true)
        .place(ui),
    );
    put(
        "shader",
        kit::Shader::new().height(40.0).label("Noise").place(ui),
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
    lua_app_at(src, frames, 1.0)
}

/// [`lua_app`] at a display scale.
fn lua_app_at(src: &str, frames: usize, scale: f32) -> (Core, Vec<String>) {
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
        let mut ui = core.frame_with(VIEW, scale, &mut exts);
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
        LuaRig {
            core,
            exts,
            warnings: Vec::new(),
            host: Vec::new(),
            app,
        }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let mut ui = self.core.frame_with(VIEW, 1.0, &mut self.exts);
            ui.configure_root(NodeSpec::column().fill());
            ui.finish();
            let events = self.core.take_pending_events();
            let host = &mut self.host;
            self.exts.route(events, |ev| host.push(ev));
            self.warnings.extend(
                self.core
                    .take_warnings()
                    .into_iter()
                    .map(|w| format!("{w:?}")),
            );
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
        let e = self
            .core
            .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
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
    assert!(
        r.text("Neutral").is_some(),
        "open after a click in the middle"
    );
    let away = r.named("Elsewhere");
    r.slow_click(away.x + away.w / 2.0, away.y + away.h / 2.0);
    r.frames(2);
    assert!(r.text("Neutral").is_none(), "closed by the press outside");
    // The press that closes a menu is the menu's, as a native one's is;
    // the next reaches the window.
    r.slow_click(away.x + away.w / 2.0, away.y + away.h / 2.0);
    r.frames(1);
    assert!(
        r.text("clicks 1").is_some(),
        "the window is not blocked once it closed"
    );
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
    let bx = r
        .node(|n| n.label.as_deref() == Some("box"))
        .expect("the box")
        .rect;
    r.text(text)
        .is_some_and(|t| t.y >= bx.y - 0.5 && t.y + t.h <= bx.y + bx.h + 0.5)
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
        let e = r
            .core
            .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
        r.route(e);
        // A short move, as a glide's first frame is: it was within a row of the
        // end, which the old test still counted as the end.
        let e = r.core.handle_input(InputEvent::ScrollGesture {
            delta: Vec2::new(0.0, 10.0),
            begins: true,
        });
        r.route(e);
        r.frames(4);
        assert!(
            !shows(&mut r, "line 60"),
            "the reader's scroll up stands (wrap {wrap})"
        );
        assert!(
            r.text("LATEST").is_some(),
            "the jump back shows while it is away (wrap {wrap})"
        );
        // Back down to the end: following again, no jump.
        let e = r.core.handle_input(InputEvent::ScrollGesture {
            delta: Vec2::new(0.0, -2000.0),
            begins: false,
        });
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
    let focused = r
        .core
        .access_tree()
        .nodes
        .iter()
        .any(|n| n.name.as_deref() == Some("Note") && n.focused);
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
        let k = r
            .node(|n| n.label.as_deref() == Some("knob"))
            .expect("the knob");
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
                    .or_else(|| {
                        nodes.iter().find(|n| {
                            n.rect.x == rest.x && n.rect.y == rest.y && n.rect.w == rest.w
                        })
                    })
                    .map(|n| n.bg)
                    .expect("the button")
            };
            let rest_bg = bg(&mut r);
            use kui_core::{InputEvent, Vec2};
            let e = r
                .core
                .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
            r.route(e);
            r.frames(3);
            let hover_bg = bg(&mut r);
            let e = r.core.handle_input(InputEvent::mouse_down(1));
            r.route(e);
            r.frames(1);
            let press_bg = bg(&mut r);
            assert_ne!(rest_bg, hover_bg, "{skin} {variant}: a hover shows");
            assert_ne!(hover_bg, press_bg, "{skin} {variant}: a press shows");
            assert_eq!(
                press_bg.a, 1.0,
                "{skin} {variant}: the press is opaque: {press_bg:?}"
            );
            assert_eq!(
                hover_bg.a, 1.0,
                "{skin} {variant}: the hover is opaque: {hover_bg:?}"
            );
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
    let between = xs
        .iter()
        .filter(|x| **x > first + 1.0 && **x < last - 1.0)
        .count();
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
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
            .or(Some("?".into()))
    };
    let press_tab = |r: &mut LuaRig| {
        use kui_core::KeyCode;
        use kui_core::testing::{key_press, tab};
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
    assert!(
        seen.iter().any(|f| f.as_deref() == Some("After")),
        "Tab got past the list: {seen:?}"
    );
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
        let face = r
            .node(|n| n.label.as_deref() == Some("face"))
            .expect("the face")
            .rect;
        let quads = kui_core::testing::quads_of(&mut r.core, kui_core::QuadKind::Shadow);
        let shadow = quads
            .first()
            .map(|q| (q.rect.x + q.rect.w, q.rect.y + q.rect.h));
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
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        hit.x + hit.w / 2.0,
        hit.y + hit.h / 2.0,
    )));
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
    assert!(
        hover.0 < rest.0 && hover.1 < rest.1,
        "lifted: {rest:?} → {hover:?}"
    );
    assert!(
        press.0 > rest.0 && press.1 > rest.1,
        "pushed in: {rest:?} → {press:?}"
    );
    assert_eq!(
        rest_shadow, hover_shadow,
        "the shadow's far edge stays put under hover"
    );
    assert!(
        press_shadow.is_none() || press_shadow == rest_shadow,
        "pressed, the shadow is under the face: {press_shadow:?}"
    );
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
    let handle = |r: &mut LuaRig| {
        r.node(|n| n.label.as_deref() == Some("handle"))
            .expect("the handle")
    };
    let rest = handle(&mut r).bg;
    let track = r.named("Zoom");
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        track.x + track.w * 0.25,
        track.y + track.h / 2.0,
    )));
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
    let outer = r
        .node(|n| n.label.as_deref() == Some("outer"))
        .unwrap()
        .rect;
    // Every one-px-tall solid (the hairline under the title) starts past
    // the 2 px frame and ends before it.
    let lines: Vec<_> = kui_core::testing::solids(&mut r.core)
        .into_iter()
        .filter(|q| q.rect.h <= 1.5 && q.rect.w > 100.0)
        .collect();
    assert!(!lines.is_empty(), "the title's rule");
    for q in lines {
        assert!(
            q.rect.x >= outer.x + 1.5 && q.rect.x + q.rect.w <= outer.x + outer.w - 1.5,
            "inside the frame: {:?} in {outer:?}",
            q.rect
        );
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
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        bx.x + 100.0,
        bx.y + 100.0,
    )));
    r.route(e);
    let e = r.core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(0.0, 400.0),
        begins: true,
    });
    r.route(e);
    r.frames(4);
    let first_seen = |r: &mut LuaRig| {
        let bx = r.node(|n| n.label.as_deref() == Some("box")).unwrap().rect;
        let mut seen: Vec<(f32, String)> = r
            .core
            .nodes()
            .into_iter()
            .filter_map(|n| {
                n.text
                    .filter(|t| t.starts_with("line "))
                    .map(|t| (n.rect.y, t))
            })
            .filter(|(y, _)| *y >= bx.y)
            .collect();
        seen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        seen.first().cloned().expect("a row in view")
    };
    let before = first_seen(&mut r);
    for drop in [false, true, true] {
        r.app
            .globals()
            .get::<mlua::Function>("tick")
            .unwrap()
            .call::<()>(drop)
            .unwrap();
        r.frames(3);
        let now = first_seen(&mut r);
        assert_eq!(now.1, before.1, "the same row at the top (drop {drop})");
        assert!(
            (now.0 - before.0).abs() < 1.0,
            "in the same place (drop {drop}): {before:?} → {now:?}"
        );
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
    let (core, w) = lua_app(
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
    assert!(
        nodes.iter().all(|n| n.kind != kui_core::NodeKind::Path),
        "no path"
    );
    let polys: Vec<_> = nodes
        .iter()
        .filter(|n| n.kind == kui_core::NodeKind::Polygon)
        .collect();
    assert!(
        polys.len() > 10,
        "the area is cut into polygons: {}",
        polys.len()
    );
    let line = nodes
        .iter()
        .filter(|n| n.kind == kui_core::NodeKind::Line)
        .max_by(|a, b| a.rect.w.total_cmp(&b.rect.w))
        .expect("the series' line")
        .rect;
    let left = polys.iter().map(|p| p.rect.x).fold(f32::MAX, f32::min);
    let right = polys
        .iter()
        .map(|p| p.rect.x + p.rect.w)
        .fold(f32::MIN, f32::max);
    assert!(
        (left - line.x).abs() <= 2.0 && (right - (line.x + line.w)).abs() <= 2.0,
        "the area spans the line: {left}..{right} vs {line:?}"
    );
    // Two polygons each ramp the edge they share, so a pixel on it is
    // covered three quarters and the fill shows a seam there: every chunk
    // but the first starts on a whole pixel (its box is a pixel out).
    let off: Vec<f32> = polys
        .iter()
        .map(|p| p.rect.x + 1.0)
        .filter(|x| *x > left + 1.5 && (x - x.round()).abs() > 1e-3)
        .collect();
    assert!(off.is_empty(), "chunks start between pixels: {off:?}");
}

#[test]
fn a_line_charts_area_chunks_meet_on_whole_device_pixels_at_a_fractional_scale() {
    // At 1.5 a whole logical pixel is half a device one every other time,
    // so the seams the cuts hide stayed, fainter. Kit reads the display
    // scale (`env.scale`, C127) and cuts on whole device pixels.
    let (core, w) = lua_app_at(
        r#"
        local cols = { t = {}, a = {} }
        for i = 1, 300 do cols.t[i], cols.a[i] = i, 50 + 30 * math.sin(i / 9) end
        local ch = channel(cols)
        function view(env)
          return column { width = "grow", height = "grow", pad = { l = 7.3 },
            component("kit/line_chart", { data = ch, x = "t", area = true, height = 160,
              series = { { field = "a", name = "A" } } }) }
        end
        "#,
        4,
        1.5,
    );
    assert!(w.is_empty(), "{w:#?}");
    let nodes = core.nodes();
    let polys: Vec<_> = nodes
        .iter()
        .filter(|n| n.kind == kui_core::NodeKind::Polygon)
        .collect();
    assert!(
        polys.len() > 10,
        "the area is cut into polygons: {}",
        polys.len()
    );
    let left = polys.iter().map(|p| p.rect.x).fold(f32::MAX, f32::min);
    // Each chunk but the first starts where the one before ended: its box
    // is a logical pixel out, and that start lands on a whole device pixel.
    let starts: Vec<f32> = polys
        .iter()
        .map(|p| p.rect.x + 1.0)
        .filter(|x| *x > left + 1.5)
        .collect();
    assert!(starts.len() > 10);
    let off: Vec<f32> = starts
        .iter()
        .map(|x| x * 1.5)
        .filter(|d| (d - d.round()).abs() > 2e-3)
        .collect();
    assert!(
        off.is_empty(),
        "chunks start between device pixels: {off:?}"
    );
    let logical_only = starts
        .iter()
        .filter(|x| (*x - x.round()).abs() > 1e-3)
        .count();
    assert!(
        logical_only > 0,
        "some cuts are on half logical pixels, as whole device ones at 1.5 are"
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
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        top.x + top.w / 2.0,
        top.y + top.h - 10.0,
    )));
    r.route(e);
    r.frames(2);
    let over_bar: i64 = r.app.globals().get("hovers").unwrap();
    assert_eq!(
        over_bar, 0,
        "a band past the scroller's edge takes no pointer"
    );
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        top.x + top.w / 2.0,
        top.y + top.h + 30.0,
    )));
    r.route(e);
    r.frames(2);
    let inside: i64 = r.app.globals().get("hovers").unwrap();
    assert!(inside > 0, "inside the scroller the bands still hover");
}

/// Every focus-ring colour a skin draws, light and dark, as quads carry it.
fn focus_colours() -> Vec<kui_core::Color> {
    [
        0x2f6fedffu32,
        0x7fa8ffff,
        0x5b8cffff,
        0x0f7a35ff,
        0x39ff6aff,
    ]
    .iter()
    .map(|&c| kui_core::Color::hex(c))
    .collect()
}

fn focus_quads(core: &mut Core) -> usize {
    let ring = focus_colours();
    kui_core::testing::solids(core)
        .iter()
        .filter(|q| {
            ring.iter()
                .any(|c| q.color == *c || (q.border_w > 0.0 && q.border_color == *c))
        })
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
                r.node(|n| n.label.as_deref() == Some("after_field"))
                    .unwrap()
                    .rect
                    .y,
                r.node(|n| n.label.as_deref() == Some("after_area"))
                    .unwrap()
                    .rect
                    .y,
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
    // its buttons, centred on the taller row, left the field's line. In a
    // toolbar with room the message sits whole beside the text; one too
    // narrow for it puts it under the field (the next test).
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
    let below = r
        .node(|n| n.label.as_deref() == Some("below"))
        .unwrap()
        .rect
        .y;
    let add = r.named("Add");
    r.app.globals().set("err", "Name the task first").unwrap();
    r.frames(3);
    assert!(r.text("Name the task first").is_some(), "the message shows");
    assert_eq!(
        r.node(|n| n.label.as_deref() == Some("below"))
            .unwrap()
            .rect
            .y,
        below,
        "the toolbar keeps its height"
    );
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
    assert_eq!(
        focus_quads(&mut r.core),
        0,
        "a click focuses without a ring"
    );
    kui_core::testing::tab(&mut r.core, true);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(2);
    let ring = r
        .node(|n| n.float && (n.rect.w - 22.0).abs() < 0.5 && (n.rect.h - 22.0).abs() < 0.5)
        .expect("the ring round the box")
        .rect;
    let bx = r
        .node(|n| {
            (n.rect.w - 14.0).abs() < 0.5
                && (n.rect.h - 14.0).abs() < 0.5
                && n.rect.x >= row.x - 0.5
        })
        .expect("the box")
        .rect;
    assert!(
        (bx.x - ring.x - 4.0).abs() < 0.5 && (bx.y - ring.y - 4.0).abs() < 0.5,
        "2 px out: {ring:?} round {bx:?}"
    );
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
    assert_eq!(
        focus_quads(&mut r.core),
        0,
        "no ring round the list after a click"
    );
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
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    r.route(e);
    r.frames(2);
    assert_eq!(
        r.core.cursor_shape(),
        CursorShape::Grab,
        "an open hand over the handle"
    );
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    r.frames(2);
    assert_eq!(
        r.core.cursor_shape(),
        CursorShape::Grabbing,
        "a closed one while it is held"
    );
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(x + 60.0, y)));
    r.route(e);
    r.frames(2);
    assert_eq!(
        r.core.cursor_shape(),
        CursorShape::Grabbing,
        "still closed off the handle"
    );
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
    let glyphs = |r: &mut LuaRig| {
        kui_core::testing::quads_of(&mut r.core, kui_core::QuadKind::GlyphMask).len()
    };
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
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Title"),
        "the title has the keyboard"
    );
    kui_core::testing::tab(&mut r.core, false);
    r.frames(2);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Note"),
        "Tab goes on to the body"
    );
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
            r.core
                .access_tree()
                .nodes
                .iter()
                .find(|n| n.focused)
                .and_then(|n| n.name.clone())
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
        let into = stops
            .iter()
            .position(|s| s.as_deref().is_some_and(|s| rows.contains(&s)));
        if !selectable {
            assert!(
                into.is_none(),
                "a table nobody selects in is no stop: {stops:?}"
            );
            continue;
        }
        assert_eq!(
            stops
                .iter()
                .filter(|s| s.as_deref().is_some_and(|s| rows.contains(&s)))
                .count(),
            1,
            "the rows are one stop: {stops:?}"
        );
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
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    let mut stops = Vec::new();
    for _ in 0..3 {
        kui_core::testing::tab(&mut r.core, false);
        r.frames(1);
        stops.push(focused(&mut r));
    }
    assert_eq!(
        stops[1].as_deref(),
        Some("a: 5%"),
        "the bars are one stop, the first bar: {stops:?}"
    );
    assert_eq!(
        stops[2].as_deref(),
        Some("After"),
        "and the next stop is past them: {stops:?}"
    );
    kui_core::testing::tab(&mut r.core, true);
    r.frames(1);
    use kui_core::{EditKey, InputEvent, Mods};
    r.core
        .handle_input(InputEvent::Key(EditKey::Right, Mods::NONE));
    r.frames(2);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("b: 2.9%"),
        "Right walks to the next bar"
    );
    let items: Vec<_> = r
        .core
        .nodes()
        .into_iter()
        .filter(|n| n.role == Some(kui_core::Role::ListItem))
        .collect();
    let (a, bar) = (items[0].clone(), items[1].clone());
    assert_ne!(bar.bg, a.bg, "the bar the keyboard is on looks hovered");
    assert!(r.text("b: 2.9%").is_some(), "its tooltip shows");
    let e = r
        .core
        .handle_input(InputEvent::Key(EditKey::Enter, Mods::NONE));
    r.route(e);
    r.frames(1);
    let picked: Option<i64> = r.app.globals().get("picked").unwrap();
    assert_eq!(picked, Some(1), "Enter picks it");
    // The value column hugs its widest label.
    let six = r.text("6%").expect("the top value label");
    assert!(
        a.rect.x - (six.x + six.w) <= 10.0,
        "the first bar sits by the labels: {six:?} {:?}",
        a.rect
    );

    let mut r = LuaRig::new(&src(false));
    r.frames(2);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(1);
    kui_core::testing::tab(&mut r.core, false);
    r.frames(1);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("After"),
        "bars nobody picks are no stop"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_waterfall_zooms_by_the_wheel_over_lanes_that_scroll_and_selects_a_range_on_its_axis() {
    // Its lanes scrolled under the wheel once they were taller than their
    // box, so the wheel zoomed only a shallow trace; and a drag selected
    // nothing.
    let src = r#"
        local name, start, len, depth, group = {}, {}, {}, {}, {}
        for d = 0, 14 do
          name[#name + 1], start[#start + 1], len[#len + 1], depth[#depth + 1], group[#group + 1] = "s" .. d, d, 100 - d, d, d
        end
        local ch = channel({ name = name, start = start, len = len, depth = depth, group = group })
        zoom, range = nil, nil
        function on_event(ev)
          if ev.kind == "zoom" then zoom = ev.zoom end
          if ev.kind == "range" then range = ev.range end
        end
        function view(env)
          return column { width = 600, height = 200,
            component("kit/waterfall", { bars = ch, start = 0, length = 100, grow = true,
              on_zoom = { kind = "zoom" }, on_range = { kind = "range" } }) }
        end
    "#;
    let mut r = LuaRig::new(src);
    r.frames(3);
    use kui_core::{InputEvent, Vec2};
    let field = |r: &LuaRig, t: &str, f: &str| -> Option<f64> {
        let v: mlua::Value = r.app.globals().get(t).unwrap();
        v.as_table().and_then(|t| t.get::<f64>(f).ok())
    };
    // The wheel down over the lanes, which overflow their box: a zoom.
    let lane = r.named("s0");
    let (x, y) = (lane.x + 20.0, lane.y + lane.h / 2.0);
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(0.0, 40.0),
        begins: true,
    });
    r.route(e);
    r.frames(1);
    let length = field(&r, "zoom", "length").expect("the wheel zoomed");
    assert!(length < 100.0, "in: {length}");
    // A swipe sideways: a pan, as long as it was.
    let e = r.core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(-60.0, 0.0),
        begins: true,
    });
    r.route(e);
    r.frames(1);
    assert!(field(&r, "zoom", "start").unwrap() > 0.0, "panned later");
    assert!((field(&r, "zoom", "length").unwrap() - 100.0).abs() < 1e-6);
    // A drag along the axis: a range.
    let tick = r.text("0 ms").expect("the axis");
    let (ax, ay) = (tick.x + 4.0, tick.y + tick.h / 2.0);
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(ax + 60.0, ay)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(ax + 200.0, ay)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_up());
    r.route(e);
    r.frames(1);
    let range = field(&r, "range", "length").expect("a range");
    assert!(range > 10.0, "{range}");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_waterfalls_bar_pans_under_a_drag_and_a_double_click_makes_its_span_the_range() {
    // A drag that began on a bar moved nothing: the bar took the press.
    let src = r#"
        local ch = channel({ name = { "load", "parse" }, start = { 0, 10 }, len = { 60, 20 },
          depth = { 0, 1 }, group = { 0, 1 } })
        zoom, range, picked = nil, nil, nil
        function on_event(ev)
          if ev.kind == "zoom" then zoom = ev.zoom end
          if ev.kind == "range" then range = ev.range end
          if ev.kind == "pick" then picked = ev.select.row end
        end
        function view(env)
          return column { width = 600, height = 200,
            component("kit/waterfall", { bars = ch, start = 0, length = 100,
              on_zoom = { kind = "zoom" }, on_range = { kind = "range" }, on_select = { kind = "pick" } }) }
        end
    "#;
    let mut r = LuaRig::new(src);
    r.frames(3);
    use kui_core::{InputEvent, Vec2};
    let field = |r: &LuaRig, t: &str, f: &str| -> Option<f64> {
        let v: mlua::Value = r.app.globals().get(t).unwrap();
        v.as_table().and_then(|t| t.get::<f64>(f).ok())
    };
    let bar = r.named("load");
    let (x, y) = (bar.x + 20.0, bar.y + bar.h / 2.0);
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(x + 120.0, y)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_up());
    r.route(e);
    r.frames(1);
    assert!(
        field(&r, "zoom", "start").expect("a pan") < 0.0,
        "dragged right, the window moves earlier"
    );
    let picked: Option<i64> = r.app.globals().get("picked").unwrap();
    assert_eq!(picked, None, "a drag is no click");
    // A double click on the parse bar: its span is the range.
    let bar = r.named("parse");
    let (x, y) = (bar.x + bar.w / 2.0, bar.y + bar.h / 2.0);
    r.slow_click(x, y);
    // The second press of a double click, as the OS counts it.
    let e = r.core.handle_input(InputEvent::mouse_down(2));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_up());
    r.route(e);
    r.frames(1);
    assert_eq!(field(&r, "range", "start"), Some(10.0));
    assert_eq!(field(&r, "range", "length"), Some(20.0));
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
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
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
        let spans = stops
            .iter()
            .filter(|s| matches!(s.as_deref(), Some("load" | "parse" | "paint")))
            .count();
        if picks {
            // The three tools, the spans once, After, round to the first.
            assert_eq!(spans, 1, "the spans are one stop: {stops:?}");
            let at = stops
                .iter()
                .position(|s| matches!(s.as_deref(), Some("load" | "parse" | "paint")))
                .unwrap();
            r.core.set_focus(None);
            for _ in 0..=at {
                kui_core::testing::tab(&mut r.core, false);
                r.frames(1);
            }
            let first = focused(&mut r);
            use kui_core::{EditKey, InputEvent, Mods};
            r.core
                .handle_input(InputEvent::Key(EditKey::Down, Mods::NONE));
            r.frames(1);
            assert_ne!(focused(&mut r), first, "an arrow walks to the next span");
            let e = r
                .core
                .handle_input(InputEvent::Key(EditKey::Enter, Mods::NONE));
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
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    // A row is the list composite's item (ADR 0070): the click focuses it,
    // and the arrows move that focus, the selection with it.
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("two"),
        "the click focuses the row"
    );
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
    assert!(
        q.y <= four.y && q.y + q.h >= four.y + four.h,
        "on the selected row: {q:?} vs {four:?}"
    );
}

#[test]
fn rings_are_in_the_skins_focus_colour() {
    // The core's ring is the theme's blue in every skin; Kit's controls
    // draw their own in the skin's focus colour, the core's turned off.
    for (skin, colours) in [
        ("phosphor", [0x0f7a35ffu32, 0x39ff6aff]),
        ("neutral", [0x2f6fedff, 0x5b8cffff]),
    ] {
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
        let skin_ring: Vec<kui_core::Color> =
            colours.iter().map(|&c| kui_core::Color::hex(c)).collect();
        let theme_ring = [
            kui_core::Theme::light().focus_ring,
            kui_core::Theme::dark().focus_ring,
        ];
        for stop in 0..5 {
            kui_core::testing::tab(&mut r.core, false);
            r.frames(2);
            let quads = kui_core::testing::solids(&mut r.core);
            let ours = quads
                .iter()
                .filter(|q| q.border_w > 0.0 && skin_ring.contains(&q.border_color))
                .count();
            let theirs = quads
                .iter()
                .filter(|q| q.border_w > 0.0 && theme_ring.contains(&q.border_color))
                .count();
            let who = r
                .core
                .access_tree()
                .nodes
                .iter()
                .find(|n| n.focused)
                .and_then(|n| n.name.clone());
            assert!(
                ours >= 1,
                "{skin}, stop {stop} ({who:?}): a ring in the skin's colour"
            );
            assert_eq!(
                theirs, 0,
                "{skin}, stop {stop} ({who:?}): no ring in the theme's"
            );
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
    let track = r
        .node(|n| n.role == Some(kui_core::Role::Slider))
        .expect("the track")
        .rect;
    let (start, len) = (track.x + 6.0, track.w - 12.0);
    let handle = r
        .node(|n| (n.rect.w - 12.0).abs() < 0.5 && (n.rect.h - 12.0).abs() < 0.5)
        .expect("the handle")
        .rect;
    let centre = handle.x + 6.0;
    assert!(
        (centre - (start + 0.5 * len)).abs() <= 1.0,
        "the handle sits on 5: {centre} vs {}",
        start + 0.5 * len
    );
    let y = track.y + track.h / 2.0;
    use kui_core::{InputEvent, Vec2};
    let drag_to = |r: &mut LuaRig, at: f32| -> f64 {
        let e = r
            .core
            .handle_input(InputEvent::CursorMoved(Vec2::new(centre, y)));
        r.route(e);
        r.frames(1);
        let e = r.core.handle_input(InputEvent::mouse_down(1));
        r.route(e);
        r.frames(1);
        let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
            start + at / 10.0 * len,
            y,
        )));
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

/// A narrow table hides its lowest-priority columns first, header and
/// cells alike and still lined up, and scrolls sideways once only the
/// columns that never hide are left and they do not fit.
#[test]
fn a_narrow_table_hides_columns_by_priority_then_scrolls_sideways() {
    let mut r = LuaRig::new(
        r#"
        w = 700
        local ch = channel({ id = { 1, 2, 3 }, name = { "alpha", "beta", "gamma" }, ms = { 10, 20, 30 }, size = { 100, 200, 300 } })
        function view(env)
          return column { width = w, height = "grow", pad = 10,
            component("kit/table", { rows = ch, grow = true, columns = {
              { field = "id", title = "Id", width = 60, priority = 2 },
              { field = "name", title = "Name", min_width = 120 },
              { field = "ms", title = "Time", width = 80, priority = 3 },
              { field = "size", title = "Size", width = 80, priority = 1 } } }) }
        end
        "#,
    );
    // Which columns show, by header, and whether each header sits over
    // its column's cells.
    let shown = |r: &mut LuaRig, w: f64| -> Vec<&'static str> {
        r.app.globals().set("w", w).unwrap();
        r.frames(3);
        let mut out = Vec::new();
        for (i, name) in ["Id", "Name", "Time", "Size"].into_iter().enumerate() {
            let h = r
                .node(|n| n.label.as_deref() == Some(&format!("h{}", i + 1)) && n.rect.w > 0.0)
                .map(|n| n.rect);
            let c = r
                .node(|n| n.label.as_deref() == Some(&format!("c{}", i + 1)) && n.rect.w > 0.0)
                .map(|n| n.rect);
            assert_eq!(
                h.is_some(),
                c.is_some(),
                "at {w}: {name}'s header and cells show together"
            );
            if let (Some(h), Some(c)) = (h, c) {
                assert!(
                    (h.x - c.x).abs() < 0.5 && (h.w - c.w).abs() < 0.5,
                    "at {w}: {name}'s header {h:?} over its cells {c:?}"
                );
                out.push(name);
            }
        }
        out
    };
    assert_eq!(
        shown(&mut r, 700.0),
        ["Id", "Name", "Time", "Size"],
        "wide: every column"
    );
    assert_eq!(
        shown(&mut r, 360.0),
        ["Id", "Name", "Time"],
        "Size goes first"
    );
    assert_eq!(shown(&mut r, 280.0), ["Name", "Time"], "then the id");
    assert_eq!(shown(&mut r, 150.0), ["Name"], "then Time, and Name never");
    let g = r
        .core
        .nodes()
        .into_iter()
        .find(|n| n.label.as_deref() == Some("sheet"))
        .expect("the sheet")
        .rect;
    let wide = r
        .core
        .nodes()
        .into_iter()
        .find(|n| n.label.as_deref() == Some("wide"))
        .expect("the scroller")
        .rect;
    assert!(
        g.w > wide.w + 1.0,
        "what does not fit scrolls sideways: the sheet {g:?} in {wide:?}"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A value moved by a click, a key or the host springs there, the handle
/// on the fill's end all the way; a drag is followed directly, a resize
/// moves it at once, and under reduced motion it jumps.
#[test]
fn a_slider_springs_to_a_new_value_but_not_on_a_resize_or_a_drag() {
    for reduced in [false, true] {
        let mut r = LuaRig::new(
            r#"
            v, w = 2, 400
            function on_event(ev) if ev.kind == "v" then v = ev.change.value end end
            function view(env)
              return column { width = w, height = "grow", pad = 20,
                component("kit/slider", { value = v, min = 0, max = 10, step = 1, label = "Level", on_change = { kind = "v" } }) }
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
        // The handle's centre, and where on the track the value puts it.
        let at = |r: &mut LuaRig| -> (f32, f32, f32) {
            let track = r
                .node(|n| n.role == Some(kui_core::Role::Slider))
                .expect("the track")
                .rect;
            let (start, len) = (track.x + 6.0, track.w - 12.0);
            let handle = r
                .node(|n| (n.rect.w - 12.0).abs() < 0.5 && (n.rect.h - 12.0).abs() < 0.5)
                .expect("the handle")
                .rect;
            let v: f64 = r.app.globals().get("v").unwrap();
            (handle.x + 6.0, start + v as f32 / 10.0 * len, start)
        };
        let fill_end = |r: &mut LuaRig| -> f32 {
            let track = r
                .node(|n| n.role == Some(kui_core::Role::Slider))
                .unwrap()
                .rect;
            let fill = r
                .node(|n| {
                    n.parent.is_some() && n.rect.h == 4.0 && (n.rect.x - track.x - 6.0).abs() < 0.5
                })
                .expect("the fill")
                .rect;
            fill.x + fill.w
        };
        let (c0, want0, _) = at(&mut r);
        assert!((c0 - want0).abs() <= 1.0, "on 2 to begin with");

        // The host moves it: on its way after a few frames, the handle on
        // the fill's end, then there.
        r.app.globals().set("v", 8).unwrap();
        r.core.set_time(1.05);
        r.frames(1);
        r.core.set_time(1.1);
        r.frames(1);
        let (mid, want, _) = at(&mut r);
        let end = fill_end(&mut r);
        if reduced {
            assert!(
                (mid - want).abs() <= 1.0,
                "under reduced motion it jumps: {mid} vs {want}"
            );
        } else {
            assert!(
                mid > c0 + 2.0 && mid < want - 2.0,
                "on its way: {c0} .. {mid} .. {want}"
            );
            assert!(
                (mid - end).abs() <= 1.0,
                "the handle rides the fill's end: {mid} vs {end}"
            );
        }
        for k in 1..=30 {
            r.core.set_time(1.1 + k as f64 / 60.0);
            r.frames(1);
        }
        let (there, want, _) = at(&mut r);
        assert!(
            (there - want).abs() <= 1.0,
            "there in the end: {there} vs {want}"
        );

        // A resize moves it at once.
        r.app.globals().set("w", 600).unwrap();
        r.core.set_time(3.0);
        r.frames(1);
        let (wide, want, _) = at(&mut r);
        assert!(
            (wide - want).abs() <= 1.0,
            "a resize does not ease it: {wide} vs {want}"
        );

        // A drag follows the pointer: each value it proposes is drawn there.
        let track = r
            .node(|n| n.role == Some(kui_core::Role::Slider))
            .unwrap()
            .rect;
        let (_, _, start) = at(&mut r);
        let len = track.w - 12.0;
        let y = track.y + track.h / 2.0;
        use kui_core::{InputEvent, Vec2};
        let e = r
            .core
            .handle_input(InputEvent::CursorMoved(Vec2::new(wide, y)));
        r.route(e);
        r.core.set_time(3.1);
        r.frames(1);
        let e = r.core.handle_input(InputEvent::mouse_down(1));
        r.route(e);
        r.core.set_time(3.2);
        r.frames(1);
        for (k, to) in [6.0f32, 4.0, 3.0].into_iter().enumerate() {
            let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
                start + to / 10.0 * len,
                y,
            )));
            r.route(e);
            r.core.set_time(3.25 + k as f64 * 0.02);
            r.frames(1);
            let (now, want, _) = at(&mut r);
            if k > 0 {
                assert!(
                    (now - want).abs() <= 1.0,
                    "a drag is followed directly (to {to}): {now} vs {want}"
                );
            }
        }
        let e = r.core.handle_input(InputEvent::mouse_up());
        r.route(e);
        r.frames(2);
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
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
    let track = r
        .node(|n| n.role == Some(kui_core::Role::Slider))
        .expect("the track")
        .rect;
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
    assert!(
        right >= track.x + track.w - 2.0,
        "it reaches the track's end: {right} vs {:?}",
        track
    );
    assert!(
        left <= track.x + 2.0,
        "and starts from its start: {left} vs {:?}",
        track
    );
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
    assert_eq!(
        asks[0].directory.as_deref(),
        Some("/Users/me/Tidewater"),
        "it starts in the field's folder"
    );
    let e = r.core.handle_input(kui_core::InputEvent::Files(vec![
        "/Volumes/Data/Sync".into(),
    ]));
    r.route(e);
    r.frames(2);
    let folder: String = r.app.globals().get("folder").unwrap();
    assert_eq!(
        folder, "/Volumes/Data/Sync",
        "the path picked is the field's"
    );
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
        let panel = r
            .node(|n| n.role == Some(kui_core::Role::Group) && n.label.as_deref() == Some("Folder"))
            .map(|n| n.rect)
            .unwrap_or_else(|| r.named("Folder"));
        let choose = r.named("Choose… sync folder");
        let mode = r.named("Mode: A rather long choice");
        let inside =
            |b: kui_core::Rect| b.x >= panel.x - 0.5 && b.x + b.w <= panel.x + panel.w - 1.5;
        assert!(
            inside(choose),
            "at {w}: the button inside the panel: {choose:?} in {panel:?}"
        );
        assert!(
            inside(mode),
            "at {w}: the select inside the panel: {mode:?} in {panel:?}"
        );
        let label = r
            .node(|n| {
                n.text
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().starts_with("choose"))
            })
            .expect("the button's label")
            .rect;
        assert!(
            label.x + label.w <= choose.x + choose.w,
            "at {w}: its whole label"
        );
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
    let e = r
        .core
        .handle_input(kui_core::InputEvent::Text("item 0995".into()));
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
    assert!(
        tree.nodes.iter().any(|n| n.role == kui_core::Role::Table),
        "a table"
    );
    assert!(
        tree.nodes
            .iter()
            .any(|n| n.role == kui_core::Role::ColumnHeader),
        "with column headers"
    );
    let focused = |r: &mut LuaRig| -> Option<String> {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    let auth = r.text("auth").unwrap();
    r.slow_click(auth.x + 2.0, auth.y + auth.h / 2.0);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("auth"));
    let sel: Option<i64> = r.app.globals().get("sel").unwrap();
    assert_eq!(
        sel,
        Some(0),
        "a click on a cell selects its row, as arrowing onto it does"
    );
    press_key(&mut r, kui_core::KeyCode::Right);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("up"),
        "Right along the row"
    );
    press_key(&mut r, kui_core::KeyCode::Down);
    r.frames(1);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("down"),
        "Down keeps the column"
    );
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
        let on = r
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone());
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
    assert!(
        xs.iter().all(|x| (*x - last).abs() < 0.5),
        "in one frame, no steps between: {xs:?}"
    );
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
    assert!(
        a.x + a.w <= b.x + 1.0 || b.x + b.w <= a.x + 1.0,
        "side by side, not overlapping: {runs:?}"
    );
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
    assert!(
        wide.len() > 11,
        "more ticks than one every 90 px: {texts:?}"
    );
    assert!(
        texts.iter().all(|t| !t.contains('.')),
        "whole steps read whole: {texts:?}"
    );
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
            let v: f64 = t
                .trim_end_matches(" ms")
                .trim_end_matches('k')
                .parse()
                .ok()?;
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
        let ys: Vec<_> = y_labels(&core)
            .into_iter()
            .filter(|(r, _)| r.x < 100.0)
            .collect();
        assert!(ys.len() >= 3, "{ys:?}");
        let right = ys.iter().map(|(r, _)| r.x + r.w).fold(f32::MIN, f32::max);
        let widest = ys.iter().map(|(r, _)| r.w).fold(f32::MIN, f32::max);
        assert!(
            right - widest <= 4.0,
            "the gutter hugs its widest label ({top}): {right} vs {widest}"
        );
        assert!(
            right < old,
            "narrower than the old fixed gutter ({top}): {right}"
        );
        for (r, _) in &ys {
            assert!((r.x + r.w - right).abs() < 1.0, "right-aligned: {ys:?}");
        }
    }
}

#[test]
fn a_line_chart_glides_to_a_new_range_and_jumps_under_reduced_motion() {
    // A range under twice the last glides; past it, it jumps: the stroke is
    // drawn through the stretch, and ten times over smeared it into a band.
    for (reduced, top_of, shown, glides) in [
        (false, 14, 15.0, true),
        (true, 14, 15.0, false),
        (false, 100, 100.0, false),
    ] {
        let mut r = LuaRig::new(&format!(
            r#"
            local function make(top)
              local t, a = {{}}, {{}}
              for i = 1, 50 do t[i] = i; a[i] = top * i / 50 end
              return channel({{ t = t, a = a }})
            end
            local small, big = make(10), make({top_of})
            ch = small
            function grow() ch = big end
            function view(env)
              return column {{ width = 600, height = "grow",
                component("kit/line_chart", {{ data = ch, x = "t", x_unit = "s", height = 160,
                  series = {{ {{ field = "a", name = "A" }} }} }}) }}
            end
            "#
        ));
        if reduced {
            let mut system = r.core.env.system;
            system.motion = kui_core::MotionPref::Reduced;
            r.core.set_system(system);
        }
        r.core.set_time(1.0);
        r.frames(3);
        let top = |r: &mut LuaRig| {
            y_labels(&r.core)
                .iter()
                .map(|(_, v)| *v)
                .fold(f64::MIN, f64::max)
        };
        assert_eq!(top(&mut r), 10.0);
        r.app
            .globals()
            .get::<mlua::Function>("grow")
            .unwrap()
            .call::<()>(())
            .unwrap();
        r.core.set_time(1.05);
        r.frames(1);
        r.core.set_time(1.1);
        r.frames(1);
        // The labels name the new range at once and slide to their places;
        // the lines are drawn for it too, in a box that glides from where
        // the old range had them (C149): its stretch down, as drawn.
        assert_eq!(top(&mut r), shown, "the labels name the new range");
        let stretch = |r: &mut LuaRig| {
            let (dl, _) = r.core.output();
            dl.quads
                .iter()
                .filter(|q| q.kind == kui_core::QuadKind::Segment)
                .map(|q| dl.clip_of(q).transform.sy)
                .fold(1.0f32, |a, b| {
                    if (b - 1.0).abs() > (a - 1.0).abs() {
                        b
                    } else {
                        a
                    }
                })
        };
        let mid = stretch(&mut r);
        if !glides {
            assert_eq!(
                mid, 1.0,
                "under reduced motion, or past twice over, the range jumps"
            );
        } else {
            assert!(
                mid > 1.0 && mid < 1.5,
                "on its way from 1.5 times as tall: {mid}"
            );
            assert!(r.core.animating(), "the core owes frames while it glides");
            assert_eq!(r.core.next_frame_at(), None, "and the view asks for none");
        }
        r.core.set_time(1.6);
        r.frames(2);
        assert_eq!(top(&mut r), shown, "there in the end (reduced {reduced})");
        assert_eq!(stretch(&mut r), 1.0);
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
    let data = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../addons/kit/lua/data.lua"
    ))
    .unwrap();
    for (name, src) in [("shimmer", SHIMMER), ("noise", NOISE)] {
        assert!(
            data.contains(src),
            "lua/data.lua carries shaders/{name}.wgsl as it is"
        );
        assert!(
            Core::new().add_fragment(src).is_some(),
            "shaders/{name}.wgsl compiles"
        );
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
        assert!(
            (lines[2].w - lines[0].w * 0.6).abs() < 1.0,
            "the last line is shorter: {lines:?}"
        );
        assert!(
            frags.iter().any(|b| b.w == 40.0 && b.h == 40.0),
            "a circle 40 across"
        );
        assert_eq!(
            r.core.animating(),
            !reduced,
            "it sweeps unless motion is reduced"
        );
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
                    kit::Shader::new()
                        .preset(preset)
                        .height(50.0)
                        .label("A preset")
                        .place(ui);
                });
            }
            let w = r.core.take_warnings();
            assert!(w.is_empty(), "{w:#?}");
            let frag = r
                .core
                .nodes()
                .into_iter()
                .find(|n| n.kind == kui_core::NodeKind::Fragment)
                .expect("the fragment");
            assert_eq!(frag.rect.h, 50.0);
            r.centre("A preset");
            assert_eq!(
                r.core.animating(),
                !reduced,
                "a preset moves unless motion is reduced ({preset:?})"
            );
        }
    }
    // WGSL handed over is registered by Kit; the host's own handle is drawn.
    let mut r = rig();
    let id = r.core.add_fragment(NOISE).unwrap();
    r.frame(|ui| {
        kit::Shader::new().source(SHIMMER).height(20.0).place(ui);
        kit::Shader::new()
            .id(id.to_ffi() as i64)
            .params(vec![0.0; 12])
            .height(21.0)
            .place(ui);
    });
    assert!(r.core.take_warnings().is_empty());
    let frags = r
        .core
        .nodes()
        .into_iter()
        .filter(|n| n.kind == kui_core::NodeKind::Fragment)
        .count();
    assert_eq!(frags, 2);
    assert!(
        !r.core.animating(),
        "a caller's shader moves only when it says animate"
    );
    // Source that does not compile is said, and the box is the well.
    let mut r = rig();
    r.frame(|ui| {
        kit::Shader::new()
            .source("fn fragment(")
            .height(30.0)
            .place(ui);
        kit::Shader::new().height(31.0).place(ui);
    });
    let w = r.core.take_warnings();
    assert!(w.iter().any(|w| w.code == "fragment-rejected"), "{w:#?}");
    assert!(
        r.core
            .nodes()
            .iter()
            .all(|n| n.kind != kui_core::NodeKind::Fragment),
        "no shader, no fragment"
    );
    assert!(
        r.core.nodes().iter().any(|n| n.rect.h == 30.0)
            && r.core.nodes().iter().any(|n| n.rect.h == 31.0),
        "the wells keep their place"
    );
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
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(fx, fy)));
    r.route(e);
    let e = r.core.handle_input(InputEvent::mouse_down(1));
    r.route(e);
    r.frames(2);
    let two = r.named("Two");
    let mid = two.y + two.h / 2.0;
    assert!(
        mid > field.y && mid < field.y + field.h,
        "the current option sits over the field: {two:?} vs {field:?}"
    );
    let three = r.named("Three");
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        fx,
        three.y + three.h / 2.0,
    )));
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
    let e = kui_core::testing::key_down(
        &mut r.core,
        kui_core::KeyCode::Char('c'),
        kui_core::KeyMods::NONE.with_primary(),
    );
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
    assert!(
        r.text("1\n2\n3").is_some(),
        "the lines' numbers in their gutter"
    );
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
    assert_eq!(
        copied,
        vec!["local a = 1\nlocal b = 2\nreturn a + b".to_string()]
    );
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
            kit::Settings::new()
                .appearance(appearance)
                .roles(roles)
                .place(ui);
            let t = |ui: &mut Ui<'_>, n: &str| ui.token_color(n).expect(n);
            seen.push((
                t(ui, "kitAccent"),
                t(ui, "kitSel"),
                t(ui, "kitOnAccent"),
                t(ui, "kitSelFg"),
                t(ui, "kitBg"),
                t(ui, "kitFg"),
            ));
        });
    }
    let (accent, sel, on, sel_fg, bg, _) = seen[0];
    assert_eq!(sel, accent, "the selection follows the accent");
    assert!(
        on.r > 0.9 && on.g > 0.9,
        "light on the dark blue, not the ink: {on:?}"
    );
    assert_eq!(sel_fg, on, "and on the selection");
    let (_, _, _, _, dark_bg, dark_fg) = seen[1];
    assert_ne!(dark_bg, bg, "dark held: the dark half's page");
    assert_ne!(dark_fg, seen[0].5, "and its text");
}

/// WCAG's contrast between two opaque colours.
fn contrast(a: kui_core::Color, b: kui_core::Color) -> f32 {
    let lum = |c: kui_core::Color| {
        let lin = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    };
    let (x, y) = (lum(a), lum(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}

/// `top`, with its alpha, laid over the opaque `under`.
fn over(under: kui_core::Color, top: kui_core::Color) -> kui_core::Color {
    let m = |u: f32, t: f32| u + (t - u) * top.a;
    kui_core::Color {
        r: m(under.r, top.r),
        g: m(under.g, top.g),
        b: m(under.b, top.b),
        a: 1.0,
    }
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
                kit::Settings::new()
                    .skin(skin)
                    .appearance(appearance)
                    .place(ui);
            });
            r.frame(|ui| {
                for n in ["kitBg", "kitPanel", "kitFg", "kitChosen", "kitAccent"] {
                    plain.insert(n, ui.token_color(n).unwrap());
                }
            });
            for accent in accents {
                let mut got = std::collections::HashMap::new();
                let names = [
                    "kitAccent",
                    "kitOnAccent",
                    "kitSel",
                    "kitSelFg",
                    "kitBg",
                    "kitSide",
                    "kitPanel",
                    "kitSunken",
                    "kitRaised",
                    "kitFg",
                    "kitMuted",
                    "kitFaint",
                    "kitFocus",
                    "kitOk",
                    "kitWarn",
                    "kitDanger",
                    "kitAccentText",
                    "kitChosen",
                    "kitChosenFg",
                    "kitGroup1",
                    "kitGroupFg1",
                    "kitSeries1",
                ];
                r.frame(|ui| {
                    let roles = Value::map([("accent", Value::Int(accent))]);
                    kit::Settings::new()
                        .skin(skin)
                        .appearance(appearance)
                        .roles(roles)
                        .place(ui);
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
                let pairs = [
                    ("kitOnAccent", "kitAccent"),
                    ("kitChosenFg", "kitChosen"),
                    ("kitGroupFg1", "kitGroup1"),
                ];
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
                assert_eq!(
                    at("kitChosen"),
                    at("kitAccent"),
                    "{case}: a view switch's chosen segment"
                );
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
        let e = r
            .core
            .handle_input(kui_core::testing::key_press(KeyCode::Tab));
        r.route(e);
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
        r
    };
    let focused = |r: &mut LuaRig| {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
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
    for (platform, opening) in [
        (Platform::MacOs, mac),
        (Platform::Windows, others),
        (Platform::Linux, others),
    ] {
        for &(code, mods) in opening {
            let mut r = rig(platform, "two");
            assert_eq!(
                focused(&mut r).as_deref(),
                Some("Mode: Two"),
                "Tab reached the field"
            );
            press(&mut r, code, mods);
            assert!(
                r.text("Three").is_some(),
                "{platform:?} {code:?} {mods:?} opened the menu"
            );
            assert_eq!(
                focused(&mut r).as_deref(),
                Some("Two"),
                "{platform:?} {code:?}: focus on the current option"
            );
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
            assert!(
                r.text("Skip").is_none(),
                "{platform:?}: {code:?} from {from} left it closed"
            );
            if let Some(to) = to {
                let label = format!("Mode: {}{}", to[..1].to_uppercase(), &to[1..]);
                assert_eq!(
                    focused(&mut r).as_deref(),
                    Some(label.as_str()),
                    "the field shows the step, focused"
                );
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
    let focused = |r: &mut LuaRig| {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    let tab = |r: &mut LuaRig, back: bool| {
        use kui_core::testing::tab;
        let mods = if back {
            kui_core::KeyMods::NONE.with_shift()
        } else {
            kui_core::KeyMods::NONE
        };
        let kp =
            kui_core::InputEvent::KeyDown(kui_core::KeyPress::new(kui_core::KeyCode::Tab, mods));
        let e = r.core.handle_input(kp);
        r.route(e);
        let e = tab(&mut r.core, back);
        r.route(e);
        r.frames(1);
    };
    tab(&mut r, false);
    tab(&mut r, false);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Mode: Two"),
        "on the select"
    );
    tab(&mut r, false);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("After"),
        "Tab leaves it forward"
    );
    tab(&mut r, true);
    assert_eq!(focused(&mut r).as_deref(), Some("Mode: Two"), "back on it");
    tab(&mut r, true);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Before"),
        "Shift-Tab leaves it back"
    );
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
        assert!(
            clip.y >= pane.y - 0.5,
            "the handle is cut at the pane's top: clip {clip:?}, pane {pane:?}"
        );
    }
}

#[test]
fn every_notice_and_toast_reads_on_its_own_ground() {
    // Neutral with a yellow accent drew its info notice yellow on the grey
    // well. Drawn, not looked up: each notice's text at 4.5:1 on its well
    // and its frame at 3:1; each toast's text on its surface at 4.5:1 and
    // its tone's bar at 3:1 — every tone, skin and half, accent or none.
    let tones = [
        kit::Tone::Info,
        kit::Tone::Neutral,
        kit::Tone::Accent,
        kit::Tone::Ok,
        kit::Tone::Warn,
        kit::Tone::Danger,
    ];
    let accents: [Option<i64>; 5] = [
        None,
        Some(0xffd400ff),
        Some(0x7fe9ffff),
        Some(0x1e2a5aff),
        Some(0xdc2626ff),
    ];
    for skin in [kit::Skin::Ledger, kit::Skin::Neutral, kit::Skin::Phosphor] {
        for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
            for accent in accents {
                let mut r = rig();
                let (mut sunken, mut raised) = (None, None);
                for _ in 0..2 {
                    r.frame(|ui| {
                        let mut s = kit::Settings::new().skin(skin).appearance(appearance);
                        if let Some(a) = accent {
                            s = s.roles(Value::map([("accent", Value::Int(a))]));
                        }
                        s.place(ui);
                        for t in tones {
                            kit::Notice::new("Saved drafts are kept for 30 days.")
                                .tone(t)
                                .place(ui);
                        }
                        for t in tones {
                            kit::Toast::new("The upstream is slow.")
                                .tone(t)
                                .inline(true)
                                .duration(0.0)
                                .place(ui);
                        }
                        sunken = ui.token_color("kitSunken");
                        raised = ui.token_color("kitRaised");
                    });
                }
                let (sunken, raised) = (sunken.unwrap(), raised.unwrap());
                let case = format!("{skin:?} {appearance:?} {accent:x?}");
                let (dl, _) = r.core.output();
                let glyphs: Vec<&kui_core::Quad> = dl
                    .quads
                    .iter()
                    .filter(|q| {
                        matches!(
                            q.kind,
                            kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
                        )
                    })
                    .collect();
                let inside = |q: &kui_core::Quad, f: kui_core::Rect| {
                    let (x, y) = (q.rect.x + q.rect.w / 2.0, q.rect.y + q.rect.h / 2.0);
                    x >= f.x && x <= f.x + f.w && y >= f.y && y <= f.y + f.h
                };
                let frames: Vec<&kui_core::Quad> = dl
                    .quads
                    .iter()
                    .filter(|q| q.kind == kui_core::QuadKind::Solid && q.border_w > 0.0)
                    .collect();
                let notices: Vec<&&kui_core::Quad> =
                    frames.iter().filter(|q| q.color == sunken).collect();
                let toasts: Vec<&&kui_core::Quad> = frames
                    .iter()
                    .filter(|q| q.color == raised && q.rect.w >= 300.0)
                    .collect();
                assert!(
                    notices.len() >= tones.len(),
                    "{case}: every notice drawn ({})",
                    notices.len()
                );
                assert!(
                    toasts.len() >= tones.len(),
                    "{case}: every toast drawn ({})",
                    toasts.len()
                );
                for n in &notices {
                    let k = contrast(n.border_color, sunken);
                    assert!(k >= 2.99, "{case}: a notice's frame reads at {k:.2}");
                    for g in glyphs.iter().filter(|g| inside(g, n.rect)) {
                        let k = contrast(g.color, sunken);
                        assert!(k >= 4.49, "{case}: a notice's text reads at {k:.2}");
                    }
                }
                for t in &toasts {
                    for g in glyphs.iter().filter(|g| inside(g, t.rect)) {
                        let k = contrast(g.color, raised);
                        assert!(k >= 4.49, "{case}: a toast's text reads at {k:.2}");
                    }
                    let bars = dl.quads.iter().filter(|q| {
                        q.kind == kui_core::QuadKind::Solid
                            && (q.rect.w - 4.0).abs() < 0.6
                            && inside(q, t.rect)
                    });
                    for b in bars {
                        let k = contrast(b.color, raised);
                        assert!(k >= 2.99, "{case}: a toast's tone bar reads at {k:.2}");
                    }
                }
            }
        }
    }
}

/// The fill under the centre of `rect` that the frame draws for it — the
/// last solid quad the size of `rect` there, laid over `ground` where it
/// is a wash — and the visible glyphs inside it.
fn fill_and_glyphs(
    core: &mut Core,
    rect: kui_core::Rect,
    ground: kui_core::Color,
) -> (kui_core::Color, Vec<kui_core::Color>) {
    let (dl, _) = core.output();
    let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let inside = |q: &kui_core::Quad| {
        let (x, y) = (q.rect.x + q.rect.w / 2.0, q.rect.y + q.rect.h / 2.0);
        x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h
    };
    let fill = dl
        .quads
        .iter()
        .rfind(|q| {
            q.kind == kui_core::QuadKind::Solid
                && q.color.a > 0.0
                && (q.rect.w - rect.w).abs() < 1.5
                && (q.rect.h - rect.h).abs() < 1.5
                && cx >= q.rect.x
                && cx <= q.rect.x + q.rect.w
                && cy >= q.rect.y
                && cy <= q.rect.y + q.rect.h
        })
        .map_or(ground, |q| over(ground, q.color));
    let glyphs = dl
        .quads
        .iter()
        .filter(|q| {
            matches!(
                q.kind,
                kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
            )
        })
        .filter(|q| q.color.a > 0.01 && inside(q))
        .map(|q| q.color)
        .collect();
    (fill, glyphs)
}

#[test]
fn a_hovered_or_pressed_menu_item_button_or_toast_action_reads_on_its_fill() {
    // C129: a danger menu item's hover painted its label in `danger` on a
    // `danger` fill, and the label vanished under the pointer. Drawn, not
    // looked up: every menu item tone hovered and pressed, a danger button
    // hovered and pressed, and a toast's action hovered and pressed — each
    // label at 4.5:1 on the fill under it, in every skin and half, accent
    // or none.
    let accents: [Option<i64>; 5] = [
        None,
        Some(0xffd400ff),
        Some(0x7fe9ffff),
        Some(0x1e2a5aff),
        Some(0xdc2626ff),
    ];
    for skin in [kit::Skin::Ledger, kit::Skin::Neutral, kit::Skin::Phosphor] {
        for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
            for accent in accents {
                let case = format!("{skin:?} {appearance:?} {accent:x?}");
                for menu in [true, false] {
                    let mut r = rig();
                    // The grounds a wash is laid over: the menu and the
                    // toast are raised, the button sits on the window.
                    let raised = std::cell::Cell::new(None);
                    let bg = std::cell::Cell::new(None);
                    let view = |r: &mut Rig| {
                        r.frame(|ui| {
                            let mut s = kit::Settings::new().skin(skin).appearance(appearance);
                            if let Some(a) = accent {
                                s = s.roles(Value::map([("accent", Value::Int(a))]));
                            }
                            s.place(ui);
                            if menu {
                                // Under an opener, as a menu is placed.
                                ui.with_keyed("opener", NodeSpec::row().size(120.0, 24.0), |ui| {
                                    kit::Menu::new(vec![
                                        kit::Item::new("open", "Open file").shortcut("⌘O"),
                                        kit::Item::new("rm", "Delete forever")
                                            .shortcut("⌘⌫")
                                            .danger(true),
                                    ])
                                    .place(ui);
                                });
                            } else {
                                kit::Button::new("Erase all")
                                    .variant(kit::Variant::Danger)
                                    .place(ui);
                                kit::Toast::new("Moved to the bin.")
                                    .action("Undo")
                                    .inline(true)
                                    .duration(0.0)
                                    .place(ui);
                            }
                            raised.set(ui.token_color("kitRaised"));
                            bg.set(ui.token_color("kitBg"));
                        });
                    };
                    view(&mut r);
                    view(&mut r);
                    let targets: &[&str] = if menu {
                        &["Open file", "Delete forever"]
                    } else {
                        &["Erase all", "Undo"]
                    };
                    for &name in targets {
                        let node = r
                            .core
                            .access_tree()
                            .nodes
                            .iter()
                            .find(|n| n.name.as_deref() == Some(name))
                            .cloned()
                            .unwrap_or_else(|| panic!("{case}: no {name}"));
                        let at = kui_core::Vec2::new(
                            node.rect.x + node.rect.w / 2.0,
                            node.rect.y + node.rect.h / 2.0,
                        );
                        for pressed in [false, true] {
                            kui_core::testing::hover(&mut r.core, at);
                            if pressed {
                                r.core.handle_input(kui_core::InputEvent::mouse_down(1));
                            }
                            view(&mut r);
                            view(&mut r);
                            let g = if menu || name == "Undo" {
                                raised.get()
                            } else {
                                bg.get()
                            };
                            let (fill, glyphs) =
                                fill_and_glyphs(&mut r.core, node.rect, g.unwrap());
                            let state = if pressed { "pressed" } else { "hovered" };
                            assert!(!glyphs.is_empty(), "{case}: {name} {state} draws its label");
                            for c in glyphs {
                                let k = contrast(c, fill);
                                assert!(
                                    k >= 4.49,
                                    "{case}: {name} {state}: {c:?} on {fill:?} reads at {k:.2}"
                                );
                            }
                            if pressed {
                                kui_core::testing::hover(
                                    &mut r.core,
                                    kui_core::Vec2::new(2.0, 2.0),
                                );
                                kui_core::testing::release(&mut r.core);
                                view(&mut r);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_checklist_is_one_tab_stop_its_arrows_walk_and_space_ticks() {
    let mut r = LuaRig::new(
        r#"
        items = { { label = "Milk", checked = false }, { label = "Eggs", checked = false }, { label = "Bread", checked = true } }
        removed = nil
        function on_event(ev)
          if ev.kind == "tick" then items[ev.toggle.index + 1].checked = ev.toggle.checked end
          if ev.kind == "rm" then removed = ev.remove.index; table.remove(items, ev.remove.index + 1) end
        end
        function view(env)
          return column { width = 300, pad = 20, gap = 8,
            component("kit/button", { label = "Before" }),
            component("kit/checklist", { items = items, label = "Groceries", removable = true,
              on_toggle = { kind = "tick" }, on_remove = { kind = "rm" } }),
            component("kit/button", { label = "After" }) }
        end
        "#,
    );
    r.frames(2);
    let focused = |r: &mut LuaRig| {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    let tab = |r: &mut LuaRig, back: bool| {
        let mods = if back {
            kui_core::KeyMods::NONE.with_shift()
        } else {
            kui_core::KeyMods::NONE
        };
        let e = r
            .core
            .handle_input(kui_core::InputEvent::KeyDown(kui_core::KeyPress::new(
                kui_core::KeyCode::Tab,
                mods,
            )));
        r.route(e);
        let e = kui_core::testing::tab(&mut r.core, back);
        r.route(e);
        r.frames(1);
    };
    let key = |r: &mut LuaRig, code: kui_core::KeyCode| {
        let e = r
            .core
            .press(kui_core::KeyPress::new(code, kui_core::KeyMods::NONE));
        r.route(e);
        if code == kui_core::KeyCode::Space {
            let e = r.core.handle_input(kui_core::InputEvent::Text(" ".into()));
            r.route(e);
        }
        r.frames(2);
    };
    tab(&mut r, false);
    tab(&mut r, false);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Milk"),
        "Tab enters on the first item"
    );
    key(&mut r, kui_core::KeyCode::Down);
    assert_eq!(focused(&mut r).as_deref(), Some("Eggs"), "Down walks");
    key(&mut r, kui_core::KeyCode::Space);
    let eggs: bool = r.app.load("return items[2].checked").eval().unwrap();
    assert!(eggs, "Space ticks the focused item");
    let node = r
        .core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Eggs"))
        .cloned()
        .unwrap();
    assert_eq!(
        node.description.as_deref(),
        Some("done"),
        "and it reads ticked"
    );
    tab(&mut r, false);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("After"),
        "one stop: Tab leaves"
    );
    tab(&mut r, true);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Eggs"),
        "Shift-Tab comes back to the item it left"
    );
    // The pointer on Eggs' ×: a press there removes it.
    let eggs = r.named("Eggs");
    r.slow_click(eggs.x + eggs.w - 12.0, eggs.y + eggs.h / 2.0);
    r.frames(1);
    let removed: Option<i64> = r.app.globals().get("removed").unwrap();
    assert_eq!(removed, Some(1), "the × removes its item");
    let bread = r.text("Bread").unwrap();
    r.click(bread.x + 2.0, bread.y + bread.h / 2.0);
    r.frames(2);
    let bread_on: bool = r.app.load("return items[2].checked").eval().unwrap();
    assert!(!bread_on, "a click unticks");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// Delete and Backspace remove the focused item (the list hears the keys
/// its items leave, C120); focus stays on the row, now the next item's,
/// and the last row's goes back one. The list stays one Tab stop.
#[test]
fn a_checklist_removes_its_focused_item_with_delete_and_backspace() {
    use kui_core::KeyCode;
    let mut r = LuaRig::new(
        r#"
        items = { { label = "Milk" }, { label = "Eggs" }, { label = "Bread" }, { label = "Tea" } }
        function on_event(ev)
          if ev.kind == "rm" then table.remove(items, ev.remove.index + 1) end
        end
        function view(env)
          return column { width = 300, pad = 20, gap = 8,
            component("kit/button", { label = "Before" }),
            component("kit/checklist", { items = items, label = "Groceries", removable = true, on_remove = { kind = "rm" } }),
            component("kit/button", { label = "After" }) }
        end
        "#,
    );
    r.frames(2);
    let focused = |r: &mut LuaRig| {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    let labels = |r: &mut LuaRig| -> String {
        r.app.load("local t = {} for i, it in ipairs(items) do t[i] = it.label end return table.concat(t, ' ')").eval().unwrap()
    };
    let tab = |r: &mut LuaRig| {
        let e = r
            .core
            .handle_input(kui_core::InputEvent::KeyDown(kui_core::KeyPress::new(
                KeyCode::Tab,
                kui_core::KeyMods::NONE,
            )));
        r.route(e);
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
    };
    let key = |r: &mut LuaRig, code: KeyCode| {
        let e = r
            .core
            .press(kui_core::KeyPress::new(code, kui_core::KeyMods::NONE));
        r.route(e);
        r.frames(2);
    };
    tab(&mut r);
    tab(&mut r);
    key(&mut r, KeyCode::Down);
    assert_eq!(focused(&mut r).as_deref(), Some("Eggs"));
    key(&mut r, KeyCode::Delete);
    assert_eq!(
        labels(&mut r),
        "Milk Bread Tea",
        "Delete removes the focused item"
    );
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Bread"),
        "focus stays on its row, now the next item"
    );
    key(&mut r, KeyCode::Backspace);
    assert_eq!(labels(&mut r), "Milk Tea", "Backspace too");
    assert_eq!(focused(&mut r).as_deref(), Some("Tea"));
    key(&mut r, KeyCode::Delete);
    r.frames(3);
    assert_eq!(labels(&mut r), "Milk", "the last item");
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Milk"),
        "focus goes back one from the last row"
    );
    key(&mut r, KeyCode::Down);
    assert_eq!(labels(&mut r), "Milk", "the arrows are still the list's");
    tab(&mut r);
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("After"),
        "still one Tab stop"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// Tab straight onto the last row and Delete before any frame has drawn
/// focus there: the case Kit's old workaround (the row's key recorded by
/// the view while focused) missed. The core hands focus to the row before
/// (C125), with nothing of Kit's in between.
#[test]
fn a_checklist_tab_straight_onto_the_last_row_then_delete_lands_on_the_row_before() {
    use kui_core::KeyCode;
    let mut r = LuaRig::new(
        r#"
        items = { { label = "Milk" }, { label = "Eggs" }, { label = "Bread" } }
        function on_event(ev)
          if ev.kind == "rm" then table.remove(items, ev.remove.index + 1) end
        end
        function view(env)
          return column { width = 300, pad = 20, gap = 8,
            component("kit/button", { label = "Before" }),
            component("kit/checklist", { items = items, label = "Groceries", removable = true, on_remove = { kind = "rm" } }),
            component("kit/button", { label = "After" }) }
        end
        "#,
    );
    r.frames(2);
    let focused = |r: &mut LuaRig| {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .and_then(|n| n.name.clone())
    };
    let labels = |r: &mut LuaRig| -> String {
        r.app.load("local t = {} for i, it in ipairs(items) do t[i] = it.label end return table.concat(t, ' ')").eval().unwrap()
    };
    // A Tab with no frame after it: focus moves, nothing draws.
    let tab = |r: &mut LuaRig, back: bool| {
        let mods = if back {
            kui_core::KeyMods::NONE.with_shift()
        } else {
            kui_core::KeyMods::NONE
        };
        let e = r
            .core
            .handle_input(kui_core::InputEvent::KeyDown(kui_core::KeyPress::new(
                KeyCode::Tab,
                mods,
            )));
        r.route(e);
        let e = kui_core::testing::tab(&mut r.core, back);
        r.route(e);
    };
    let key = |r: &mut LuaRig, code: KeyCode| {
        let e = r
            .core
            .press(kui_core::KeyPress::new(code, kui_core::KeyMods::NONE));
        r.route(e);
    };
    // Onto Bread, the last row, and out to After.
    tab(&mut r, false);
    tab(&mut r, false);
    r.frames(1);
    key(&mut r, KeyCode::End);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("Bread"));
    tab(&mut r, false);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("After"));
    // Shift-Tab straight back onto Bread, and Delete at once.
    tab(&mut r, true);
    key(&mut r, KeyCode::Delete);
    r.frames(2);
    assert_eq!(labels(&mut r), "Milk Eggs", "Delete removed the last row");
    assert_eq!(
        focused(&mut r).as_deref(),
        Some("Eggs"),
        "and focus went to the row before"
    );
    // Again from the far side: Before, Tab into the list (entered on Eggs,
    // now its last row), Delete at once.
    tab(&mut r, true);
    r.frames(1);
    assert_eq!(focused(&mut r).as_deref(), Some("Before"));
    tab(&mut r, false);
    key(&mut r, KeyCode::Delete);
    r.frames(2);
    assert_eq!(labels(&mut r), "Milk");
    assert_eq!(focused(&mut r).as_deref(), Some("Milk"));
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A line chart of ten rows (x 0..9, A = 10x, B = 100 - x), focusable.
fn readout_rig() -> LuaRig {
    LuaRig::new(
        r#"
        local t, a, b = {}, {}, {}
        for i = 0, 9 do t[#t + 1] = i; a[#a + 1] = i * 10; b[#b + 1] = 100 - i end
        local ch = channel({ t = t, a = a, b = b })
        function view(env)
          return column { width = 640, height = "grow", pad = 20, gap = 8,
            component("kit/button", { label = "Before" }),
            component("kit/line_chart", { data = ch, x = "t", height = 160, focusable = true,
              series = { { field = "a", name = "A" }, { field = "b", name = "B" } } }) }
        end
        "#,
    )
}

fn readout(r: &mut LuaRig) -> Option<String> {
    r.core
        .access_tree()
        .nodes
        .iter()
        .find(|n| {
            n.live != kui_core::Live::Off && n.name.as_deref().is_some_and(|s| s.contains(", A "))
        })
        .and_then(|n| n.name.clone())
}

#[test]
fn a_line_chart_reads_out_the_sample_under_the_pointer_and_hides_on_leave() {
    let mut r = readout_rig();
    r.frames(3);
    assert!(readout(&mut r).is_none(), "nothing until the pointer comes");
    let plot = r
        .node(|n| n.label.as_deref() == Some("plot"))
        .expect("the plot")
        .rect;
    use kui_core::{InputEvent, Vec2};
    // Over row 5 (x 5 of 0..9 is 5/9 of the way across).
    let at = |r: &mut LuaRig, f: f32| {
        let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
            plot.x + plot.w * f,
            plot.y + plot.h / 2.0,
        )));
        r.route(e);
        r.frames(3);
    };
    at(&mut r, 5.0 / 9.0);
    assert_eq!(
        readout(&mut r).as_deref(),
        Some("5, A 50, B 95"),
        "x and each series' value"
    );
    at(&mut r, 2.0 / 9.0);
    assert_eq!(
        readout(&mut r).as_deref(),
        Some("2, A 20, B 98"),
        "it follows the pointer"
    );
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        plot.x + plot.w / 2.0,
        plot.y + plot.h + 200.0,
    )));
    r.route(e);
    r.frames(3);
    assert!(readout(&mut r).is_none(), "gone when the pointer leaves");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// The readout lands on the sample nearest the pointer by the x scale the
/// line is drawn with: in a window the data fills 40% of (x -13.5..9, rows
/// at 0..9 — a live line's first seconds) as in a full one, near the left
/// edge with the gutter at its width, and its box clear of the samples'
/// heights, beside the crosshair.
#[test]
fn a_line_chart_reads_out_the_nearest_sample_in_a_partly_filled_window() {
    for (window, cases) in [
        // 40% filled: the empty span reads the first sample, then the data.
        (
            Some(22.5),
            vec![
                (0.02, "0, A 0, B 100"),
                (0.6, "0, A 0, B 100"),
                (0.6 + 0.4 * 5.0 / 9.0, "5, A 50, B 95"),
                (0.995, "9, A 90, B 91"),
            ],
        ),
        // Full: the edges.
        (
            None,
            vec![
                (0.01, "0, A 0, B 100"),
                (0.5 / 9.0 + 0.01, "1, A 10, B 99"),
                (7.0 / 9.0, "7, A 70, B 93"),
                (0.995, "9, A 90, B 91"),
            ],
        ),
    ] {
        let mut r = LuaRig::new(&format!(
            r#"
            local t, a, b = {{}}, {{}}, {{}}
            for i = 0, 9 do t[#t + 1] = i; a[#a + 1] = i * 10; b[#b + 1] = 100 - i end
            local ch = channel({{ t = t, a = a, b = b }})
            function view(env)
              return column {{ width = 640, height = "grow", pad = 20,
                component("kit/line_chart", {{ data = ch, x = "t", height = 160, window = {window}, focusable = true,
                  series = {{ {{ field = "a", name = "A" }}, {{ field = "b", name = "B" }} }} }}) }}
            end
            "#,
            window = window.map_or("nil".to_string(), |w: f64| w.to_string())
        ));
        r.frames(6);
        use kui_core::{InputEvent, Vec2};
        for (f, want) in cases {
            let plot = r
                .node(|n| n.label.as_deref() == Some("plot"))
                .expect("the plot")
                .rect;
            let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
                plot.x + plot.w * f,
                plot.y + plot.h / 2.0,
            )));
            r.route(e);
            r.frames(3);
            assert_eq!(
                readout(&mut r).as_deref(),
                Some(want),
                "window {window:?}, {:.0}% across",
                f * 100.0
            );
            // The box keeps clear of both samples' heights, not only their dots.
            let bx = r
                .node(|n| n.label.as_deref() == Some("readout"))
                .map(|n| n.rect);
            assert!(bx.is_some(), "the readout box");
            let dots: Vec<_> = r
                .core
                .nodes()
                .into_iter()
                .filter(|n| n.rect.w == 7.0 && n.rect.h == 7.0)
                .map(|n| n.rect)
                .collect();
            assert_eq!(dots.len(), 2, "a dot per series");
            if let Some(bx) = bx {
                for d in dots {
                    let (cx, cy) = (d.x + 3.5, d.y + 3.5);
                    assert!(
                        !(cy >= bx.y && cy <= bx.y + bx.h),
                        "the box {bx:?} is level with a sample at y {cy}"
                    );
                    assert!(
                        !(cx >= bx.x && cx <= bx.x + bx.w),
                        "the box {bx:?} is over the crosshair at x {cx}"
                    );
                    assert!(
                        bx.x >= plot.x && bx.x + bx.w <= plot.x + plot.w,
                        "the box {bx:?} flips to stay in the plot"
                    );
                }
            }
        }
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_focusable_line_chart_steps_its_readout_with_the_keyboard() {
    let mut r = readout_rig();
    r.frames(2);
    let tab = |r: &mut LuaRig| {
        let e = r
            .core
            .handle_input(kui_core::testing::key_press(kui_core::KeyCode::Tab));
        r.route(e);
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
    };
    let key = |r: &mut LuaRig, code: kui_core::KeyCode| {
        let e = r
            .core
            .press(kui_core::KeyPress::new(code, kui_core::KeyMods::NONE));
        r.route(e);
        r.frames(2);
    };
    tab(&mut r);
    tab(&mut r);
    let focused = r
        .core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .and_then(|n| n.name.clone());
    assert_eq!(
        focused.as_deref(),
        Some("line chart"),
        "the plot is a Tab stop"
    );
    // A slider over the samples resting on the newest: Left steps back
    // from it, Right on to it.
    key(&mut r, kui_core::KeyCode::Left);
    assert_eq!(
        readout(&mut r).as_deref(),
        Some("8, A 80, B 92"),
        "Left steps back from the newest"
    );
    key(&mut r, kui_core::KeyCode::Right);
    assert_eq!(
        readout(&mut r).as_deref(),
        Some("9, A 90, B 91"),
        "Right steps on to it"
    );
    key(&mut r, kui_core::KeyCode::Home);
    assert_eq!(
        readout(&mut r).as_deref(),
        Some("0, A 0, B 100"),
        "Home to the first"
    );
    key(&mut r, kui_core::KeyCode::Right);
    assert_eq!(
        readout(&mut r).as_deref(),
        Some("1, A 10, B 99"),
        "Right steps on"
    );
    // It is a slider to a screen reader, the readout its value.
    let plot = r
        .core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .cloned()
        .expect("the focused plot");
    assert_eq!(plot.role, kui_core::Role::Slider);
    assert_eq!(plot.value.as_deref(), Some("1, A 10, B 99"), "{plot:?}");
    // No key sink: Tab goes on from it, Shift-Tab back.
    tab(&mut r);
    let focused = r
        .core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .and_then(|n| n.name.clone());
    assert_eq!(
        focused.as_deref(),
        Some("Before"),
        "Tab leaves the chart (round to the first stop)"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

#[test]
fn a_bar_and_a_sparkline_show_the_value_under_the_pointer() {
    let mut r = LuaRig::new(
        r#"
        local bars = channel({ name = { "a", "b", "c" }, n = { 10, 20, 30 } })
        local trend = channel({ v = { 1, 2, 3, 4, 5, 6, 7, 8 } })
        function view(env)
          return column { width = 600, height = "grow", pad = 20, gap = 20,
            row { key = "bars", label = "bars", width = "grow",
              component("kit/bar_chart", { data = bars, label = "name", value = "n", height = 120, values = false }) },
            row { key = "spark", label = "spark", width = 160,
              component("kit/sparkline", { data = trend, field = "v", width = 160, height = 24, unit = "ms" }) } }
        end
        "#,
    );
    r.frames(2);
    use kui_core::{InputEvent, Vec2};
    let move_to = |r: &mut LuaRig, x: f32, y: f32| {
        let e = r
            .core
            .handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
        r.route(e);
        r.frames(3);
    };
    let b = r
        .node(|n| n.label.as_deref() == Some("b2"))
        .expect("the second bar")
        .rect;
    move_to(&mut r, b.x + b.w / 2.0, b.y + b.h - 4.0);
    assert!(
        r.text("b: 20").is_some(),
        "the bar under the pointer says its value"
    );
    let spark = r
        .node(|n| n.label.as_deref() == Some("spark"))
        .unwrap()
        .rect;
    move_to(&mut r, spark.x + spark.w - 2.0, spark.y + spark.h / 2.0);
    move_to(&mut r, spark.x + spark.w - 3.0, spark.y + spark.h / 2.0);
    assert!(
        r.text("8 ms").is_some(),
        "the sparkline says the sample under the pointer"
    );
    // Sample i is drawn at (i - 1) / 7 of the width: 77% across is nearest
    // the sixth (71%), though it is in the seventh eighth of the samples.
    move_to(
        &mut r,
        spark.x + spark.w * 0.77 + 1.0,
        spark.y + spark.h / 2.0,
    );
    move_to(&mut r, spark.x + spark.w * 0.77, spark.y + spark.h / 2.0);
    assert!(
        r.text("6 ms").is_some(),
        "the sample nearest the pointer, where the line draws it"
    );
    move_to(&mut r, 5.0, 5.0);
    assert!(
        r.text("8 ms").is_none(),
        "and nothing once the pointer leaves"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A live window of `n` samples, x 1..n, A = 7x mod 101 (so neighbours
/// read differently), republished from `ch` when there is one.
fn live_samples(ch: Option<&Channel>, n: usize) -> Channel {
    let t: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let a: Vec<f64> = (1..=n).map(|i| ((i * 7) % 101) as f64).collect();
    let cols = vec![("t".into(), Column::F64(t)), ("a".into(), Column::F64(a))];
    match ch {
        Some(ch) => ch.publish(cols).unwrap(),
        None => Channel::table(cols).unwrap(),
    }
}

fn live_readout(core: &mut Core) -> Option<String> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| {
            n.live != kui_core::Live::Off && n.name.as_deref().is_some_and(|s| s.contains(", A "))
        })
        .and_then(|n| n.name.clone())
}

/// The sliding box's label: `slide<n>`, keyed by the sample that started
/// its slide.
fn slide_of(core: &Core) -> Option<String> {
    core.nodes()
        .into_iter()
        .find_map(|n| n.label.filter(|l| l.starts_with("slide") && l.len() > 5))
}

/// A live line chart scrolls between samples instead of stepping: each
/// sample starts a slide of its content (a keyframe cycle of the core's,
/// keyed by the sample) from a sample's width right of its place to its
/// place, the frames between run no view (the fill is replayed), and a
/// window that stops moving stops asking for frames once its slide ends.
/// A pointer held still over it keeps its crosshair at the pointer while
/// the readout names whichever sample slides under it. Under reduced
/// motion it steps, and nothing slides.
#[test]
fn a_live_line_chart_slides_between_samples_and_its_crosshair_stays_at_a_still_pointer() {
    for reduced in [false, true] {
        let mut r = rig();
        if reduced {
            reduce_motion(&mut r.core);
        }
        let mut n = 30;
        let mut ch = live_samples(None, n);
        let mut t = 1.0;
        let mut answers: Vec<Option<FillAnswer>> = Vec::new();
        let frame = |r: &mut Rig, ch: &Channel, t: f64, answers: &mut Vec<Option<FillAnswer>>| {
            r.core.set_time(t);
            r.frame(|ui| {
                ui.with(NodeSpec::column().size(700.0, 300.0), |ui| {
                    answers.push(
                        kit::LineChart::new(ch.clone(), "t", vec![kit::Series::new("a", "A")])
                            .window(20.0)
                            .area(true)
                            .focusable(true)
                            .place(ui),
                    );
                });
            });
            let ev = r.core.take_pending_events();
            r.exts.route(ev, |_| {});
        };
        // Samples every 100 ms, so the chart learns the pace.
        for _ in 0..4 {
            frame(&mut r, &ch, t, &mut answers);
            t += 0.1;
            n += 1;
            ch = live_samples(Some(&ch), n);
        }
        frame(&mut r, &ch, t, &mut answers);
        let plot = r
            .core
            .nodes()
            .into_iter()
            .find(|n| n.label.as_deref() == Some("plot"))
            .unwrap()
            .rect;
        use kui_core::{InputEvent, Vec2};
        let px = plot.x + plot.w * 0.5;
        let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
            px,
            plot.y + plot.h / 2.0,
        )));
        r.exts.route(e, |_| {});
        frame(&mut r, &ch, t + 0.01, &mut answers);
        frame(&mut r, &ch, t + 0.02, &mut answers);
        let dot_x = |core: &mut Core| quads_sized(core, 7.0, 7.0).first().map(|q| q.rect.x + 3.5);
        assert!(
            live_readout(&mut r.core).is_some(),
            "a readout under the pointer"
        );
        assert_eq!(
            dot_x(&mut r.core),
            Some(px),
            "the crosshair's dot at the pointer"
        );
        let slide = slide_of(&r.core);
        assert_eq!(
            slide.is_some(),
            !reduced,
            "a slide while samples come (reduced {reduced}): {slide:?}"
        );
        // Between two samples: no view runs, and under full motion the
        // core owes the frames of the slide.
        // The line's segments, where they are drawn: their mean x moves
        // left as the slide runs, and stays under reduced motion.
        let line_x = |core: &mut Core| {
            let segs = kui_core::testing::quads_of(core, kui_core::QuadKind::Segment);
            segs.iter().map(|q| q.rect.x).sum::<f32>() / segs.len().max(1) as f32
        };
        answers.clear();
        frame(&mut r, &ch, t + 0.04, &mut answers);
        let early = line_x(&mut r.core);
        frame(&mut r, &ch, t + 0.07, &mut answers);
        let later = line_x(&mut r.core);
        if reduced {
            assert_eq!(early, later, "nothing slides under reduced motion");
        } else {
            assert!(
                later < early - 0.5,
                "the content slides left between samples: {early} → {later}"
            );
        }
        assert!(
            answers.iter().all(|a| *a == Some(FillAnswer::Replayed)),
            "no view between samples: {answers:?}"
        );
        assert_eq!(
            r.core.animating(),
            !reduced,
            "frames are owed while it slides, none under reduced motion"
        );
        assert_eq!(slide_of(&r.core), slide, "the same slide");
        // A sample landing draws what the slide ended on: every segment of
        // the line in the plot's middle is where it was a moment before.
        if !reduced {
            frame(&mut r, &ch, t + 0.099, &mut answers);
            let segs =
                |core: &mut Core| kui_core::testing::quads_of(core, kui_core::QuadKind::Segment);
            let before = segs(&mut r.core);
            t += 0.1;
            n += 1;
            ch = live_samples(Some(&ch), n);
            frame(&mut r, &ch, t, &mut answers);
            let after = segs(&mut r.core);
            let mid = |q: &kui_core::Quad| {
                q.rect.x > plot.x + plot.w * 0.15 && q.rect.x < plot.x + plot.w * 0.85
            };
            let moved: Vec<_> = before
                .iter()
                .filter(|q| mid(q) && q.rect.w > 1.5)
                .filter(|q| {
                    !after.iter().any(|a| {
                        (a.rect.x - q.rect.x).abs() < 0.75 && (a.rect.y - q.rect.y).abs() < 0.75
                    })
                })
                .map(|q| q.rect)
                .collect();
            let compared = before.iter().filter(|q| mid(q) && q.rect.w > 1.5).count();
            assert!(compared > 20, "segments compared: {compared}");
            assert!(
                moved.is_empty(),
                "the line jumped as the sample landed: {moved:?}"
            );
        }
        // The next samples: the readout moves on under the still pointer,
        // whose crosshair does not.
        let mut seen = vec![live_readout(&mut r.core).expect("a readout")];
        for _ in 0..3 {
            t += 0.1;
            n += 1;
            ch = live_samples(Some(&ch), n);
            frame(&mut r, &ch, t, &mut answers);
            assert_eq!(
                dot_x(&mut r.core),
                Some(px),
                "the crosshair stays at the pointer"
            );
            seen.push(live_readout(&mut r.core).expect("a readout"));
        }
        assert!(
            seen.windows(2).all(|w| w[0] != w[1]),
            "the sample under the pointer changes with each: {seen:?}"
        );
        if !reduced {
            assert_ne!(
                slide_of(&r.core),
                slide,
                "each sample starts a slide of its own"
            );
        }
        // A paused feed: once the last slide has run its course, no frame
        // is owed.
        frame(&mut r, &ch, t + 0.5, &mut answers);
        frame(&mut r, &ch, t + 0.6, &mut answers);
        assert!(
            !r.core.animating(),
            "a paused chart asks for no frames (reduced {reduced})"
        );
        assert!(r.core.take_warnings().is_empty());
    }
}

/// An app names its own nodes inside a Kit component (backlog C134): a box
/// and an editor it declared as a panel's children answer `layout_of`,
/// `edit_text` and `set_edit_text` by their labels, as the ones outside
/// the panel always did — the children were filed under the panel's
/// placement, which the app's view never asks in. And `pointer_in`
/// (backlog C119) says where the pointer is over one of them.
#[test]
fn an_app_finds_its_nodes_inside_a_kit_component_by_label() {
    let mut r = LuaRig::new(
        r#"
        n = 0
        seen = {}
        function view(env)
          n = n + 1
          local inside, outside = env.layout_of("inside"), env.layout_of("outside")
          seen.inside = inside and inside.w
          seen.outside = outside and outside.w
          seen.text = env.edit_text("note")
          local at = env.pointer_in("inside")
          seen.px, seen.pw = at and at.x, at and at.w
          if n == 2 then env.set_edit_text("note", "set by label") end
          return column { width = 600, height = "grow", pad = 20,
            column { key = "outside", width = 100, height = 20, on_layout = { kind = "noop" } },
            component("kit/panel", { title = "Panel" }, {
              column { key = "inside", width = 120, height = 30, hoverable = true },
              edit { key = "note", label = "Note", initial = "seeded" } }) }
        end
        "#,
    );
    r.frames(3);
    let seen: mlua::Table = r.app.globals().get("seen").unwrap();
    assert_eq!(seen.get::<Option<f64>>("outside").unwrap(), Some(100.0));
    assert_eq!(
        seen.get::<Option<f64>>("inside").unwrap(),
        None,
        "no on_layout, no layout_of"
    );
    assert_eq!(
        seen.get::<Option<String>>("text").unwrap().as_deref(),
        Some("set by label")
    );
    let inside = r
        .node(|n| n.label.as_deref() == Some("inside"))
        .expect("the box")
        .rect;
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        inside.x + 30.0,
        inside.y + 10.0,
    )));
    r.route(e);
    r.frames(1);
    let seen: mlua::Table = r.app.globals().get("seen").unwrap();
    assert_eq!(
        seen.get::<Option<f64>>("px").unwrap(),
        Some(30.0),
        "the pointer 30 px into the box"
    );
    assert_eq!(
        seen.get::<Option<f64>>("pw").unwrap(),
        Some(120.0),
        "and the box's width"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A Kit button with an icon and a tooltip, focused by Tab and drawn again
/// from its kept fill, shows its hint under a key of its own: the hint was
/// auto-keyed after the replayed children and took the icon's key, two
/// nodes under one (`duplicate-key`).
#[test]
fn a_kit_buttons_hint_shown_by_tab_keeps_a_key_of_its_own() {
    let mut r = LuaRig::new(
        r#"
        function view(env)
          return column { width = 600, height = "grow", pad = 20,
            component("kit/panel", { title = "Live line" }, {
              column { width = "grow", height = 40 },
              actions = { component("kit/button", { label = "Source", icon = "code", variant = "ghost",
                name = "Source of Live line", tooltip = "Show the file" }) } }) }
        end
        "#,
    );
    r.frames(2);
    let e = kui_core::testing::tab(&mut r.core, false);
    r.route(e);
    r.frames(3);
    assert!(
        r.core.access_tree().nodes.iter().any(|n| n.focused),
        "the button has the keyboard"
    );
    assert!(r.text("Show the file").is_some(), "its hint shows");
    let mut keys: Vec<u64> = r.core.nodes().iter().map(|n| n.key.0).collect();
    let all = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), all, "a key a node");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A danger menu row draws its label and its chord once each — the label
/// in `danger` and the chord in `faint` at rest, both `on_danger` hovered: the row's hover group sets the box's
/// `textColor` (C130), where the label was drawn twice and one copy shown.
#[test]
fn a_danger_menu_rows_label_is_one_text_recoloured_by_the_row() {
    let mut r = rig();
    let danger = std::cell::Cell::new(None);
    let on_danger = std::cell::Cell::new(None);
    let faint = std::cell::Cell::new(None);
    let view = |r: &mut Rig| {
        r.frame(|ui| {
            kit::Settings::new()
                .skin(kit::Skin::Ledger)
                .appearance(kit::Appearance::Light)
                .place(ui);
            ui.with_keyed("opener", NodeSpec::row().size(120.0, 24.0), |ui| {
                kit::Menu::new(vec![
                    kit::Item::new("open", "Open file").shortcut("⌘O"),
                    kit::Item::new("rm", "Delete forever")
                        .shortcut("⌘⌫")
                        .danger(true),
                ])
                .place(ui);
            });
            danger.set(ui.token_color("kitDanger"));
            on_danger.set(ui.token_color("kitOnDanger"));
            faint.set(ui.token_color("kitFaint"));
        });
    };
    view(&mut r);
    view(&mut r);
    let texts = r.texts();
    for t in ["Delete forever", "⌘⌫", "Open file", "⌘O"] {
        assert_eq!(
            texts.iter().filter(|s| *s == t).count(),
            1,
            "{t} drawn once: {texts:?}"
        );
    }
    let row = r
        .core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Delete forever"))
        .cloned()
        .expect("the danger row");
    let glyphs = |core: &mut Core| -> Vec<kui_core::Color> {
        let (dl, _) = core.output();
        let mut out = Vec::new();
        for q in &dl.quads {
            let inside = q.rect.y >= row.rect.y && q.rect.y + q.rect.h <= row.rect.y + row.rect.h;
            if inside
                && matches!(
                    q.kind,
                    kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
                )
                && !out.contains(&q.color)
            {
                out.push(q.color);
            }
        }
        out
    };
    assert_eq!(
        glyphs(&mut r.core),
        vec![danger.get().unwrap(), faint.get().unwrap()],
        "at rest, the label in danger and the chord faint"
    );
    let at = kui_core::Vec2::new(row.rect.x + row.rect.w / 2.0, row.rect.y + row.rect.h / 2.0);
    kui_core::testing::hover(&mut r.core, at);
    view(&mut r);
    view(&mut r);
    assert_eq!(
        glyphs(&mut r.core),
        vec![on_danger.get().unwrap()],
        "hovered, on its fill"
    );
    assert!(r.core.take_warnings().is_empty());
}

/// What is drawn under point `p`: every solid quad over it laid in order on
/// `ground`, and the rect of the topmost (the row's wash, when one is
/// drawn there) — skipping a quad smaller than a glyph, a mark or a rule.
fn wash_at(
    core: &mut Core,
    p: kui_core::Vec2,
    ground: kui_core::Color,
) -> (kui_core::Color, kui_core::Rect) {
    let (dl, _) = core.output();
    let mut fill = ground;
    let mut top = kui_core::Rect::new(0.0, 0.0, 0.0, 0.0);
    for q in &dl.quads {
        let r = q.rect;
        if q.kind == kui_core::QuadKind::Solid
            && q.color.a > 0.0
            && r.w >= 16.0
            && r.h >= 14.0
            && p.x >= r.x
            && p.x <= r.x + r.w
            && p.y >= r.y
            && p.y <= r.y + r.h
        {
            fill = over(fill, q.color);
            top = r;
        }
    }
    (fill, top)
}

/// Every glyph drawn inside `rect`, each with what lies under its middle:
/// the solid quads painted before it there, laid in order on `ground` — a
/// checkbox's tick on its box, a badge's text on its pill, the rest on the
/// row's wash.
fn glyphs_on(
    core: &mut Core,
    rect: kui_core::Rect,
    ground: kui_core::Color,
) -> Vec<(kui_core::Color, kui_core::Color)> {
    let (dl, _) = core.output();
    let mut out: Vec<(kui_core::Color, kui_core::Color)> = Vec::new();
    for (i, q) in dl.quads.iter().enumerate() {
        let (x, y) = (q.rect.x + q.rect.w / 2.0, q.rect.y + q.rect.h / 2.0);
        if !(matches!(
            q.kind,
            kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
        ) && q.color.a > 0.01
            && x >= rect.x
            && x <= rect.x + rect.w
            && y >= rect.y
            && y <= rect.y + rect.h)
        {
            continue;
        }
        let mut under = ground;
        for s in &dl.quads[..i] {
            let r = s.rect;
            if s.kind == kui_core::QuadKind::Solid
                && s.color.a > 0.0
                && r.w >= 6.0
                && r.h >= 6.0
                && x >= r.x
                && x <= r.x + r.w
                && y >= r.y
                && y <= r.y + r.h
            {
                under = over(under, s.color);
            }
        }
        if !out.contains(&(q.color, under)) {
            out.push((q.color, under));
        }
    }
    out
}

/// Every row Kit washes on hover and press — a list's, a table's, a tab's,
/// a segment's, a menu's, a select's options, a log's lines, a checklist's
/// items — hovered and then pressed in every skin, both halves and five
/// accents: each text on the row, `faint` and `muted` included, reads at
/// 4.5:1 on the wash under it (backlog C131). A menu row's shortcut in
/// `faint` read 3.7:1 on Ledger dark's hover, `muted` 4.36 on Phosphor
/// dark's with a red accent.
#[test]
fn faint_and_muted_text_reads_on_every_hovered_and_pressed_row() {
    use kui_core::{InputEvent, Vec2};
    let accents: [Option<i64>; 5] = [
        None,
        Some(0xffd400ff),
        Some(0x7fe9ffff),
        Some(0x1e2a5aff),
        Some(0xdc2626ff),
    ];
    let choices = || {
        vec![
            kit::Choice::new("a", "Alpha").count(4),
            kit::Choice::new("b", "Beta").count(12),
            kit::Choice::new("c", "Gamma"),
        ]
    };
    let mut failures: Vec<String> = Vec::new();
    for skin in [kit::Skin::Ledger, kit::Skin::Neutral, kit::Skin::Phosphor] {
        for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
            for accent in accents {
                let case = format!("{skin:?} {appearance:?} {accent:x?}");
                // Two scenes, so a float opened by one is not over the
                // other's rows: the rows on the page, and the menus.
                for scene in 0..4 {
                    let mut r = rig();
                    let bg = std::cell::Cell::new(None);
                    let view = |r: &mut Rig| {
                        r.frame(|ui| {
                            let mut s = kit::Settings::new().skin(skin).appearance(appearance);
                            if let Some(a) = accent {
                                s = s.roles(Value::map([("accent", Value::Int(a))]));
                            }
                            s.place(ui);
                            match scene {
                                0 => {
                                    kit::List::new(rows(), "name")
                                        .detail("ms")
                                        .on_select(msg("l"))
                                        .place(ui);
                                    kit::Table::new(
                                        vec![
                                            kit::Column::new("name", "Name"),
                                            kit::Column::new("ms", "Time").mono(true).width(80.0),
                                        ],
                                        rows(),
                                    )
                                    .on_select(msg("t"))
                                    .place(ui);
                                    kit::Log::new(logs()).on_select(msg("g")).place(ui);
                                }
                                1 => {
                                    kit::Tabs::new(
                                        vec![
                                            kit::Tab::new("a", "First").count(3),
                                            kit::Tab::new("b", "Second").count(9),
                                        ],
                                        "a",
                                    )
                                    .place_with(ui, |_, _| {});
                                    kit::Segmented::new(choices(), "a")
                                        .label("Filter")
                                        .place(ui);
                                    kit::Checklist::new(vec![
                                        kit::Check::new("Milk", true),
                                        kit::Check::new("Eggs", false),
                                    ])
                                    .place(ui);
                                }
                                2 => {
                                    ui.with_keyed(
                                        "opener",
                                        NodeSpec::row().size(120.0, 24.0),
                                        |ui| {
                                            kit::Menu::new(vec![
                                                kit::Item::new("open", "Open file").shortcut("⌘O"),
                                                kit::Item::new("rm", "Delete forever")
                                                    .shortcut("⌘⌫")
                                                    .danger(true),
                                            ])
                                            .place(ui);
                                        },
                                    );
                                }
                                _ => {
                                    kit::Select::new(choices())
                                        .value("a")
                                        .label("Mode")
                                        .place(ui);
                                }
                            }
                            bg.set(ui.token_color("kitBg"));
                        });
                    };
                    view(&mut r);
                    view(&mut r);
                    let targets: &[&str] = match scene {
                        0 => &["beta", "12:00:02"],
                        1 => &["Second", "Beta", "Eggs", "Milk"],
                        2 => &["Open file", "Delete forever"],
                        _ => &["Beta", "Gamma"],
                    };
                    if scene == 3 {
                        // The select open, its options a menu of their own.
                        let field = r
                            .core
                            .nodes()
                            .into_iter()
                            .find(|n| {
                                n.text
                                    .as_deref()
                                    .is_some_and(|t| t.eq_ignore_ascii_case("Alpha"))
                            })
                            .map(|n| n.rect)
                            .expect("the select's value");
                        let p = Vec2::new(field.x + 4.0, field.y + field.h / 2.0);
                        let e = kui_core::testing::click(&mut r.core, p);
                        r.exts.route(e, |_| {});
                        view(&mut r);
                        view(&mut r);
                    }
                    for &name in targets {
                        let Some(at) = r
                            .core
                            .nodes()
                            .into_iter()
                            .rfind(|n| {
                                n.text
                                    .as_deref()
                                    .is_some_and(|t| t.eq_ignore_ascii_case(name))
                            })
                            .map(|n| n.rect)
                        else {
                            failures.push(format!("{case}: no {name}"));
                            continue;
                        };
                        let p = Vec2::new(at.x + 2.0, at.y + at.h / 2.0);
                        for pressed in [false, true] {
                            kui_core::testing::hover(&mut r.core, p);
                            if pressed {
                                r.core.handle_input(InputEvent::mouse_down(1));
                            }
                            view(&mut r);
                            view(&mut r);
                            let (_, wash) = wash_at(&mut r.core, p, bg.get().unwrap());
                            let state = if pressed { "pressed" } else { "hovered" };
                            for (c, fill) in glyphs_on(&mut r.core, wash, bg.get().unwrap()) {
                                let k = contrast(c, fill);
                                if k < 4.49 {
                                    failures.push(format!(
                                        "{case}: {name} {state}: {c:?} on {fill:?} reads at {k:.2}"
                                    ));
                                }
                            }
                            if pressed {
                                // Let go away from the row: a release over it
                                // would pick it, and a select's would close.
                                kui_core::testing::hover(&mut r.core, Vec2::new(1199.0, 899.0));
                                r.core.handle_input(InputEvent::mouse_up());
                                view(&mut r);
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A live line chart's readout box is opaque and over everything in the
/// plot, in every skin and half: each sample's slide opens a float of its
/// own, and floats stack in the order they opened, so the lines slid over
/// a readout box opened before them and showed through it. Its fill is
/// opaque, every segment under it is drawn before it, it does not move
/// with the slide, and its text reads at 4.5:1 on it.
#[test]
fn a_live_charts_readout_is_opaque_and_over_its_lines_in_every_skin() {
    use kui_core::{InputEvent, QuadKind, Vec2};
    for skin in [kit::Skin::Ledger, kit::Skin::Neutral, kit::Skin::Phosphor] {
        for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
            let case = format!("{skin:?} {appearance:?}");
            let mut r = rig();
            let ground = std::cell::Cell::new(None);
            let frame = |r: &mut Rig, ch: &Channel, t: f64| {
                r.core.set_time(t);
                r.frame(|ui| {
                    kit::Settings::new()
                        .skin(skin)
                        .appearance(appearance)
                        .place(ui);
                    ui.with(NodeSpec::column().size(700.0, 300.0), |ui| {
                        kit::LineChart::new(ch.clone(), "t", vec![kit::Series::new("a", "A")])
                            .window(20.0)
                            .area(true)
                            .focusable(true)
                            .place(ui);
                    });
                    ground.set(ui.token_color("kitBg"));
                });
                let ev = r.core.take_pending_events();
                r.exts.route(ev, |_| {});
            };
            let mut n = 30;
            let mut ch = live_samples(None, n);
            let mut t = 1.0;
            for _ in 0..4 {
                frame(&mut r, &ch, t);
                t += 0.1;
                n += 1;
                ch = live_samples(Some(&ch), n);
            }
            let plot = r
                .core
                .nodes()
                .into_iter()
                .find(|n| n.label.as_deref() == Some("plot"))
                .unwrap()
                .rect;
            let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
                plot.x + plot.w * 0.3,
                plot.y + plot.h * 0.5,
            )));
            r.exts.route(e, |_| {});
            // Samples land after the readout opened: each opens a slide.
            for _ in 0..3 {
                frame(&mut r, &ch, t);
                frame(&mut r, &ch, t + 0.04);
                t += 0.1;
                n += 1;
                ch = live_samples(Some(&ch), n);
            }
            frame(&mut r, &ch, t);
            frame(&mut r, &ch, t + 0.03);
            let bx = r
                .core
                .nodes()
                .into_iter()
                .find(|n| n.label.as_deref() == Some("readout"))
                .expect("a readout")
                .rect;
            let (dl, _) = r.core.output();
            let fill_at = dl
                .quads
                .iter()
                .rposition(|q| {
                    q.kind == QuadKind::Solid
                        && (q.rect.w - bx.w).abs() < 1.5
                        && (q.rect.h - bx.h).abs() < 1.5
                        && (q.rect.x - bx.x).abs() < 1.5
                })
                .unwrap_or_else(|| panic!("{case}: the readout's fill"));
            let fill = dl.quads[fill_at].color;
            assert_eq!(
                fill.a, 1.0,
                "{case}: the readout's fill is opaque: {fill:?}"
            );
            let overlaps = |q: &kui_core::Quad| {
                q.rect.x < bx.x + bx.w
                    && q.rect.x + q.rect.w > bx.x
                    && q.rect.y < bx.y + bx.h
                    && q.rect.y + q.rect.h > bx.y
            };
            let late: Vec<usize> = dl
                .quads
                .iter()
                .enumerate()
                .filter(|(i, q)| *i > fill_at && q.kind == QuadKind::Segment && overlaps(q))
                .map(|(i, _)| i)
                .collect();
            assert!(
                late.is_empty(),
                "{case}: segments drawn over the readout: {late:?}"
            );
            let glyphs = glyphs_on(&mut r.core, bx, ground.get().unwrap());
            assert!(!glyphs.is_empty(), "{case}: the readout says something");
            for (c, under) in glyphs {
                let k = contrast(c, under);
                assert!(k >= 4.49, "{case}: {c:?} on {under:?} reads at {k:.2}");
            }
            assert!(r.core.take_warnings().is_empty());
        }
    }
}

/// Spans of six groups, one a group, side by side and wide enough to be
/// labelled.
fn six_spans() -> Channel {
    let names = ["load", "parse", "style", "layout", "paint", "idle"];
    Channel::table(vec![
        ("name".into(), Column::strs(names)),
        (
            "start".into(),
            Column::F64((0..6).map(|i| i as f64 * 100.0).collect()),
        ),
        ("len".into(), Column::F64(vec![95.0; 6])),
        ("depth".into(), Column::I64(vec![0; 6])),
        ("group".into(), Column::I64((0..6).collect())),
        ("target".into(), Column::strs(names)),
    ])
    .unwrap()
}

/// A waterfall's marks read in every skin, half and accent: each group's
/// bar at 3:1 on the panel, neighbouring groups apart in lightness as well
/// as hue, every label at 4.5:1 on its bar, the selected bar's edge at 3:1
/// on the panel and its label at 4.5:1 on it, a time mark at 3:1. Dark
/// navy and forest bars on near-black read at 1.3:1.
#[test]
fn a_waterfalls_marks_read_in_every_skin_half_and_accent() {
    use kui_core::QuadKind;
    let accents: [Option<i64>; 5] = [
        None,
        Some(0xffd400ff),
        Some(0x7fe9ffff),
        Some(0x1e2a5aff),
        Some(0xdc2626ff),
    ];
    let marks = Channel::table(vec![
        ("at".into(), Column::F64(vec![250.0])),
        ("label".into(), Column::strs(["paint"])),
    ])
    .unwrap();
    let mut failures: Vec<String> = Vec::new();
    for skin in [kit::Skin::Ledger, kit::Skin::Neutral, kit::Skin::Phosphor] {
        for appearance in [kit::Appearance::Light, kit::Appearance::Dark] {
            for accent in accents {
                let case = format!("{skin:?} {appearance:?} {accent:x?}");
                let mut r = rig();
                let panel = std::cell::Cell::new(None);
                let view = |r: &mut Rig| {
                    r.frame(|ui| {
                        let mut s = kit::Settings::new().skin(skin).appearance(appearance);
                        if let Some(a) = accent {
                            s = s.roles(Value::map([("accent", Value::Int(a))]));
                        }
                        s.place(ui);
                        ui.with(NodeSpec::column().width(900.0), |ui| {
                            kit::Waterfall::new(six_spans())
                                .marks(marks.clone())
                                .selected(4)
                                .on_select(msg("span"))
                                .place(ui);
                        });
                        panel.set(ui.token_color("kitPanel"));
                    });
                };
                view(&mut r);
                view(&mut r);
                let panel = panel.get().unwrap();
                let (dl, _) = r.core.output();
                let quads = dl.quads.clone();
                let mut fills = Vec::new();
                for name in ["load", "parse", "style", "layout", "paint", "idle"] {
                    let Some(node) = r
                        .core
                        .access_tree()
                        .nodes
                        .iter()
                        .find(|n| n.name.as_deref() == Some(name))
                        .cloned()
                    else {
                        failures.push(format!("{case}: no {name}"));
                        continue;
                    };
                    let rect = node.rect;
                    let Some(q) = quads.iter().rev().find(|q| {
                        q.kind == QuadKind::Solid
                            && q.color.a > 0.0
                            && (q.rect.w - rect.w).abs() < 1.5
                            && (q.rect.h - rect.h).abs() < 1.5
                            && (q.rect.x - rect.x).abs() < 1.5
                    }) else {
                        failures.push(format!("{case}: no fill for {name}"));
                        continue;
                    };
                    let selected = name == "paint";
                    if selected {
                        let k = contrast(q.border_color, panel);
                        if k < 2.99 {
                            failures.push(format!(
                                "{case}: the selected bar's edge on the panel at {k:.2}"
                            ));
                        }
                    } else {
                        let k = contrast(q.color, panel);
                        if k < 2.99 {
                            failures.push(format!("{case}: {name}'s bar on the panel at {k:.2}"));
                        }
                        fills.push(q.color);
                    }
                    for (c, under) in glyphs_on(&mut r.core, rect, panel) {
                        let k = contrast(c, under);
                        if k < 4.49 {
                            failures.push(format!("{case}: {name}'s label at {k:.2}"));
                        }
                    }
                }
                // Neighbouring groups differ in lightness, not hue alone.
                for w in fills.windows(2) {
                    let k = contrast(w[0], w[1]);
                    if k < 1.4 {
                        failures.push(format!(
                            "{case}: neighbouring groups at {k:.2} apart in lightness"
                        ));
                    }
                }
                // The time mark: a one-px rule down the lanes.
                let mark = quads.iter().find(|q| {
                    q.kind == QuadKind::Solid && (q.rect.w - 1.0).abs() < 0.1 && q.rect.h > 20.0
                });
                match mark {
                    Some(q) if contrast(q.color, panel) < 2.99 => failures.push(format!(
                        "{case}: the time mark at {:.2}",
                        contrast(q.color, panel)
                    )),
                    None => failures.push(format!("{case}: no time mark")),
                    _ => {}
                }
                assert!(r.core.take_warnings().is_empty(), "{case}");
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A Lua host keeping a Kit list's (or table's) selection as Kit proposes
/// it, and logging each event it hears as a line: `sel <row> [<ranges>
/// <count> <rows or ->]`, `cur <row>`, `act <row>`.
fn selection_rig(component: &str, selection: &str, on_focus: bool, n: usize) -> LuaRig {
    LuaRig::new(&format!(
        r#"
        local names = {{}}
        for i = 1, {n} do names[i] = "row " .. (i - 1) end
        local ch = channel({{ name = names }})
        log, selected, ranges = {{}}, nil, {{}}
        local function said(rs)
          local t = {{}}
          for _, r in ipairs(rs) do t[#t + 1] = r.first .. "-" .. r.last end
          return table.concat(t, ",")
        end
        function on_event(ev)
          if ev.kind == "sel" then
            local s = ev.select
            if s.ranges then
              ranges = s.ranges
              log[#log + 1] = "sel " .. s.row .. " " .. said(s.ranges) .. " " .. s.count .. " " .. (s.rows and #s.rows or "-")
            else
              selected = s.row
              log[#log + 1] = "sel " .. s.row
            end
          elseif ev.kind == "cur" then log[#log + 1] = "cur " .. ev.cursor.row
          elseif ev.kind == "act" then log[#log + 1] = "act " .. ev.activate.row end
        end
        function view(env)
          return column {{ width = "grow", height = "grow",
            component("kit/button", {{ label = "Before" }}),
            column {{ width = 300, height = 200,
              component("kit/{component}", {{ rows = ch, title = "name", columns = {{ {{ field = "name", title = "Name" }} }},
                selection = "{selection}", select_on_focus = {on_focus}, selected = selected, selected_ranges = ranges,
                grow = true, on_select = {{ kind = "sel" }}, on_cursor = {{ kind = "cur" }}, on_activate = {{ kind = "act" }} }}) }} }}
        end
        "#
    ))
}

/// The log since the last call.
fn heard(r: &mut LuaRig) -> Vec<String> {
    let log: mlua::Table = r.app.globals().get("log").unwrap();
    let out: Vec<String> = log
        .sequence_values::<String>()
        .map(|v| v.unwrap())
        .collect();
    r.app
        .globals()
        .set("log", r.app.create_table().unwrap())
        .unwrap();
    out
}

fn key_with(r: &mut LuaRig, code: kui_core::KeyCode, mods: kui_core::KeyMods, text: Option<&str>) {
    let mut kp = kui_core::KeyPress::new(code, mods);
    if let Some(t) = text {
        kp = kp.with_text(t);
    }
    let edit = kp.edit_event();
    let e = r
        .core
        .handle_input(kui_core::InputEvent::KeyDown(kp.clone()));
    r.route(e);
    if let Some(ev) = edit {
        let e = r.core.handle_input(ev);
        r.route(e);
    }
    let e = r
        .core
        .handle_input(kui_core::InputEvent::KeyUp(kp.released()));
    r.route(e);
    r.frames(2);
}

/// The primary modifier: ⌘ on a Mac, Ctrl elsewhere.
fn primary() -> kui_core::KeyMods {
    let mut m = kui_core::KeyMods::default();
    if cfg!(target_os = "macos") {
        m.super_key = true;
    } else {
        m.ctrl = true;
    }
    m
}

/// A click on row `name` with `mods` held, `clicks` the press's count.
fn click_row(r: &mut LuaRig, name: &str, mods: kui_core::KeyMods, clicks: u8) {
    use kui_core::{InputEvent, Vec2};
    let at = r.text(name).unwrap_or_else(|| panic!("no {name}"));
    let p = Vec2::new(at.x + 2.0, at.y + at.h / 2.0);
    for ev in [
        InputEvent::Modifiers(mods),
        InputEvent::CursorMoved(p),
        InputEvent::mouse_down(clicks),
        InputEvent::mouse_up(),
        InputEvent::Modifiers(kui_core::KeyMods::default()),
    ] {
        let e = r.core.handle_input(ev);
        r.route(e);
    }
    r.frames(2);
}

/// Tab onto the rows: the button before them, then the rows.
fn tab_in(r: &mut LuaRig) {
    r.core.set_focus(None);
    r.frames(1);
    for _ in 0..2 {
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
    }
}

/// `select_on_focus = false`: the arrows move the cursor and select
/// nothing; Space selects, Enter activates, a click selects, a double
/// click activates — in a list and a table alike.
#[test]
fn a_cursor_that_does_not_select_moves_alone_and_space_and_a_click_select() {
    use kui_core::{KeyCode, KeyMods};
    for component in ["list", "table"] {
        let mut r = selection_rig(component, "single", false, 20);
        r.frames(2);
        tab_in(&mut r);
        heard(&mut r);
        key_with(&mut r, KeyCode::Down, KeyMods::default(), None);
        key_with(&mut r, KeyCode::Down, KeyMods::default(), None);
        assert_eq!(
            heard(&mut r),
            ["cur 1", "cur 2"],
            "{component}: the cursor moves, nothing is selected"
        );
        key_with(&mut r, KeyCode::Space, KeyMods::default(), Some(" "));
        assert_eq!(
            heard(&mut r),
            ["sel 2"],
            "{component}: Space selects the cursor's row"
        );
        key_with(&mut r, KeyCode::Enter, KeyMods::default(), None);
        assert_eq!(heard(&mut r), ["act 2"], "{component}: Enter activates it");
        click_row(&mut r, "row 5", KeyMods::default(), 1);
        assert_eq!(
            heard(&mut r),
            ["cur 5", "sel 5"],
            "{component}: a click moves the cursor and selects"
        );
        click_row(&mut r, "row 5", KeyMods::default(), 2);
        assert_eq!(
            heard(&mut r),
            ["act 5"],
            "{component}: the second click of a double click activates"
        );
        let selected: i64 = r.app.globals().get("selected").unwrap();
        assert_eq!(selected, 5);
        let row = r
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("row 5"))
            .cloned()
            .unwrap();
        assert!(
            row.selected == Some(true),
            "{component}: a screen reader hears the row selected"
        );
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

/// The default, `select_on_focus = true`: the arrows select as they move,
/// as they always did, each move a cursor and a selection.
#[test]
fn a_cursor_that_selects_selects_as_it_moves() {
    use kui_core::{KeyCode, KeyMods};
    let mut r = selection_rig("list", "single", true, 20);
    r.frames(2);
    tab_in(&mut r);
    heard(&mut r);
    key_with(&mut r, KeyCode::Down, KeyMods::default(), None);
    assert_eq!(heard(&mut r), ["cur 1", "sel 1"]);
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A multiple selection: a click selects a row alone, ⌘-click (Ctrl-click
/// off a Mac) toggles one, Shift-click extends from the row last clicked,
/// Shift+Down extends from the anchor, Space toggles the cursor's row; each
/// change proposes the ranges, their count and the rows.
#[test]
fn a_multiple_selection_toggles_extends_and_ranges() {
    use kui_core::{KeyCode, KeyMods};
    for component in ["list", "table"] {
        let mut r = selection_rig(component, "multiple", false, 20);
        r.frames(2);
        click_row(&mut r, "row 1", KeyMods::default(), 1);
        assert_eq!(
            heard(&mut r),
            ["cur 1", "sel 1 1-1 1 1"],
            "{component}: a click, the row alone"
        );
        click_row(&mut r, "row 3", primary(), 1);
        assert_eq!(
            heard(&mut r),
            ["cur 3", "sel 3 1-1,3-3 2 2"],
            "{component}: ⌘-click adds a row"
        );
        click_row(&mut r, "row 6", KeyMods::default().with_shift(), 1);
        assert_eq!(
            heard(&mut r),
            ["cur 6", "sel 6 3-6 4 4"],
            "{component}: Shift-click, the range from the anchor"
        );
        click_row(&mut r, "row 4", primary(), 1);
        assert_eq!(
            heard(&mut r),
            ["cur 4", "sel 4 3-3,5-6 3 3"],
            "{component}: ⌘-click takes one out"
        );
        // The keyboard: Shift+Down extends from the anchor (row 4).
        key_with(&mut r, KeyCode::Down, KeyMods::default().with_shift(), None);
        assert_eq!(
            heard(&mut r),
            ["cur 5", "sel 5 4-5 2 2"],
            "{component}: Shift+Down extends"
        );
        key_with(&mut r, KeyCode::Down, KeyMods::default(), None);
        assert_eq!(
            heard(&mut r),
            ["cur 6"],
            "{component}: a plain arrow moves the cursor only"
        );
        key_with(&mut r, KeyCode::Space, KeyMods::default(), Some(" "));
        assert_eq!(
            heard(&mut r),
            ["sel 6 4-6 3 3"],
            "{component}: Space toggles the cursor's row in"
        );
        let ranges: mlua::Table = r.app.globals().get("ranges").unwrap();
        assert_eq!(ranges.raw_len(), 1);
        for (name, sel) in [
            ("row 3", false),
            ("row 4", true),
            ("row 6", true),
            ("row 7", false),
        ] {
            let n = r
                .core
                .access_tree()
                .nodes
                .iter()
                .find(|n| n.name.as_deref() == Some(name))
                .cloned()
                .unwrap();
            assert_eq!(
                n.selected == Some(true),
                sel,
                "{component}: {name} selected: {sel}"
            );
        }
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

/// ⌘A (Ctrl+A off a Mac) on a windowed list of 100,000 rows selects every
/// row as one range: the count, and no list of 100,000 rows.
#[test]
fn select_all_on_a_windowed_list_is_one_range() {
    use kui_core::KeyCode;
    let mut r = selection_rig("list", "multiple", true, 100_000);
    r.frames(2);
    tab_in(&mut r);
    heard(&mut r);
    key_with(&mut r, KeyCode::Char('a'), primary(), None);
    assert_eq!(heard(&mut r), ["sel 0 0-99999 100000 -"]);
    let built = r
        .core
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.selected == Some(true))
        .count();
    assert!(
        built > 0 && built < 100,
        "the rows in view draw selected: {built}"
    );
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// `selection = "none"`: the rows are still the keyboard's to walk and
/// Enter's to activate, and nothing is ever selected.
#[test]
fn a_list_that_selects_nothing_still_walks_and_activates() {
    use kui_core::{KeyCode, KeyMods};
    let mut r = selection_rig("list", "none", true, 10);
    r.frames(2);
    tab_in(&mut r);
    heard(&mut r);
    key_with(&mut r, KeyCode::Down, KeyMods::default(), None);
    key_with(&mut r, KeyCode::Enter, KeyMods::default(), None);
    click_row(&mut r, "row 4", KeyMods::default(), 1);
    assert_eq!(heard(&mut r), ["cur 1", "act 1", "cur 4"]);
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// Kit's Markdown: headings, a paragraph joined from its lines, inline
/// code, bold, italic and a link drawn as its text, a nested list, a
/// quote, a rule, a fenced block drawn as `code` and a pipe table whose
/// rows line up — every text there, no markup left in sight, no warning.
#[test]
fn markdown_draws_a_document_in_the_skin() {
    let doc = "# Kit\n\nA kit for **dense** tool UIs,\nwritten `once` and *placed* from [every](https://example.com) binding.\n\n## Use\n\n- one\n  - two, nested\n- three\n1. first\n\n> A quote.\n\n---\n\n```lua\nkit.button({ label = \"Run\" })\n```\n\n| Prop | Type |\n|---|---|\n| `label` | string |\n| `name` | string? |\n";
    let mut r = rig();
    r.frame(|ui| {
        kit::Markdown::new(doc).place(ui);
    });
    r.frame(|ui| {
        kit::Markdown::new(doc).place(ui);
    });
    let texts = r.texts().join("\n");
    for want in [
        "Kit",
        "Use",
        // The snapshot cuts a text at 60 characters.
        "A kit for dense tool UIs, written once and placed from every",
        "one",
        "two, nested",
        "three",
        "first",
        "A quote.",
        "kit.button({ label = \"Run\" })",
        "Prop",
        "label",
        "string?",
    ] {
        assert!(texts.contains(want), "{want:?} drawn:\n{texts}");
    }
    for gone in ["**", "](", "```", "|---", "https://example.com", "# "] {
        assert!(!texts.contains(gone), "{gone:?} left as markup:\n{texts}");
    }
    // The table's cells line up: `label` and `string` share a column each.
    let nodes = r.core.nodes();
    let at = |s: &str| {
        nodes
            .iter()
            .find(|n| n.text.as_deref() == Some(s))
            .map(|n| n.rect)
            .unwrap_or_else(|| panic!("no {s}"))
    };
    assert_eq!(at("Prop").x, at("label").x, "the first column lines up");
    assert_eq!(at("Type").x, at("string?").x, "the second column lines up");
    let w = r.core.take_warnings();
    assert!(w.is_empty(), "{w:#?}");
}

/// Where the leaving toast's close button is drawn against its face: the
/// × glyph's x less the face's, and the face's x, while the face is in
/// the band `y0..y1`.
fn close_against_face(core: &mut Core, y0: f32, y1: f32) -> Option<(f32, f32)> {
    let (dl, _) = core.output();
    let band: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.rect.w < 900.0 && q.rect.y >= y0 && q.rect.y + q.rect.h <= y1)
        .collect();
    let face = band
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.h > 30.0)
        .max_by(|a, b| a.rect.w.total_cmp(&b.rect.w))?;
    let close = band
        .iter()
        .filter(|q| {
            q.kind == kui_core::QuadKind::GlyphMask
                && (q.rect.w - 12.0).abs() < 0.5
                && (q.rect.h - 12.0).abs() < 0.5
        })
        .max_by(|a, b| a.rect.x.total_cmp(&b.rect.x))?;
    Some((close.rect.x - face.rect.x, face.rect.x))
}

#[test]
fn a_leaving_toasts_close_button_keeps_its_place_on_its_face() {
    // When a toast went by itself, the node that raises its dismiss came
    // into its row on its last frame and took a gap there: the close
    // button moved 10 px left on the toast's first exit frame, then slid
    // (seen only in a recording). Gone by its duration, and dismissed by a
    // click on its × (the pointer still over it, hovered and pressed), the
    // × stays where it was on the face from rest to the end of the exit.
    for by_click in [false, true] {
        let mut r = LuaRig::new(
            r#"
            shown = { 1, 2, 3 }
            function on_event(ev) if ev.kind == "off" then shown = { 1, 3 } end end
            function view(env)
              local t = {}
              for _, id in ipairs(shown) do
                t[#t + 1] = component("kit/toast", { key = "t" .. id, message = "Saved " .. id .. ".", tone = "ok",
                  inline = true, duration = id == 2 and 1500 or 0, on_dismiss = { kind = "off" } })
              end
              return column { width = "grow", height = "grow",
                component("kit/settings", { skin = "ledger" }),
                #t > 0 and component("kit/toaster", {}, t) or nil }
            end
            "#,
        );
        let mut t = 1.0;
        r.core.set_time(t);
        r.frames(3);
        t = 2.0;
        r.core.set_time(t);
        r.frames(2);
        let mid = r
            .core
            .access_tree()
            .nodes
            .iter()
            .filter(|n| n.name.as_deref() == Some("Dismiss"))
            .nth(1)
            .expect("the middle toast's close button")
            .rect;
        let (y0, y1) = (mid.y - 20.0, mid.y + mid.h + 20.0);
        let (rest, at) = close_against_face(&mut r.core, y0, y1).expect("the toast at rest");
        let mut seen = vec![rest];
        if by_click {
            r.slow_click(mid.x + mid.w / 2.0, mid.y + mid.h / 2.0);
        } else {
            t = 2.6;
            r.core.set_time(t);
        }
        let mut last = at;
        for _ in 0..16 {
            r.frames(1);
            if let Some((dx, x)) = close_against_face(&mut r.core, y0, y1) {
                seen.push(dx);
                last = x;
            }
            t += 0.016;
            r.core.set_time(t);
        }
        assert!(
            last > at + 100.0,
            "it slid out (by click {by_click}): {at} -> {last}"
        );
        assert!(
            seen.iter().all(|dx| (dx - rest).abs() < 0.01),
            "the × against the face, from rest through the exit (by click {by_click}): {seen:?}"
        );
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
    }
}

#[test]
fn a_fields_error_is_laid_out_whole_at_any_width() {
    // A version field's error read "⚠ Three numbers: 1…": cut short at the
    // end of the well, on the value's line. The whole message shows, every
    // glyph of it inside the field's column: beside the text while the well
    // has room, under the field and wrapping when it has not, the rows
    // under the field moved down to make room. A field with a Choose…
    // button, a text area, and a field in a settings row that stacks below
    // 640 px (the preferences sample's) the same.
    const MSG: &str = "Three numbers, as in 1.4.0: major, minor and patch";
    let glyphs = MSG.chars().filter(|c| !c.is_whitespace()).count();
    for skin in ["ledger", "neutral"] {
        for kind in ["field", "browse", "area", "setting"] {
            for w in [150.0f32, 230.0, 420.0, 900.0] {
                let mut r = LuaRig::new(&format!(
                    r#"
                    MSG = "{MSG}"
                    function view(env)
                      local f
                      if "{kind}" == "field" then
                        f = component("kit/text_field", {{ label = "Version", value = "1.4", mono = true, error = MSG }})
                      elseif "{kind}" == "browse" then
                        f = component("kit/text_field", {{ label = "Folder", value = "/tmp", browse = "folder", error = MSG }})
                      elseif "{kind}" == "area" then
                        f = component("kit/text_area", {{ label = "Notes", value = "x", error = MSG }})
                      else
                        f = row {{ width = "grow", gap = 24,
                          when = {{ {{ width = {{ below = 640, band = 16 }}, dir = "column", gap = 8 }} }},
                          column {{ width = 250, text("Device name") }},
                          column {{ width = "grow",
                            component("kit/text_field", {{ label = "Device name", hide_label = true, value = "x", width = 320,
                              error = MSG }}) }} }}
                      end
                      return column {{ width = "grow", height = "grow",
                        component("kit/settings", {{ skin = "{skin}" }}),
                        column {{ key = "col", label = "col", width = {w}, f }},
                        row {{ key = "below", label = "below", width = 10, height = 10 }} }}
                    end
                    "#
                ));
                r.frames(4);
                let at = format!("{skin}, {kind}, {w} px");
                let col = r.node(|n| n.label.as_deref() == Some("col")).unwrap().rect;
                let shown: Vec<_> = r
                    .core
                    .nodes()
                    .into_iter()
                    .filter(|n| n.text.as_deref() == Some(MSG) && n.rect.w > 0.0 && n.rect.h > 0.0)
                    .collect();
                assert_eq!(shown.len(), 1, "{at}: the message shows once: {shown:#?}");
                let msg = shown[0].rect;
                assert!(
                    msg.x >= col.x - 0.5 && msg.x + msg.w <= col.x + col.w + 0.5,
                    "{at}: the message inside the field's column: {msg:?} in {col:?}"
                );
                let drawn = kui_core::testing::quads_of(&mut r.core, kui_core::QuadKind::GlyphMask)
                    .into_iter()
                    .filter(|q| {
                        let (cx, cy) = (q.rect.x + q.rect.w / 2.0, q.rect.y + q.rect.h / 2.0);
                        cx >= msg.x && cx <= msg.x + msg.w && cy >= msg.y && cy <= msg.y + msg.h
                    })
                    .count();
                assert_eq!(
                    drawn, glyphs,
                    "{at}: every glyph of the message, none cut short: {msg:?}"
                );
                let below = r
                    .node(|n| n.label.as_deref() == Some("below"))
                    .unwrap()
                    .rect;
                assert!(
                    below.y >= msg.y + msg.h,
                    "{at}: what follows is under the message: {below:?} {msg:?}"
                );
                let edit = r
                    .core
                    .nodes()
                    .into_iter()
                    .find(|n| n.kind == kui_core::NodeKind::Edit)
                    .unwrap()
                    .rect;
                let beside = msg.y < edit.y + edit.h;
                match (kind, w) {
                    ("area", _) | (_, 150.0) => {
                        assert!(!beside, "{at}: under the field: {msg:?} {edit:?}")
                    }
                    ("field", 900.0) => {
                        assert!(beside, "{at}: beside the text, with room: {msg:?} {edit:?}")
                    }
                    _ => {}
                }
                assert!(r.warnings.is_empty(), "{at}: {:#?}", r.warnings);
            }
        }
    }
}

/// The gallery's feed: a sample every 0.1 of x, x accumulated as the feed
/// adds it (`t = t + 0.1`, so not exactly k / 10), the last 600 kept; CPU
/// on a wave, memory on another.
fn feed_samples(ch: Option<&Channel>, n: usize) -> Channel {
    let (mut t, mut cpu, mut mem) = (Vec::new(), Vec::new(), Vec::new());
    let mut x = 0.0f64;
    for i in 0..n {
        x += 0.1;
        t.push(x);
        cpu.push(40.0 + 25.0 * (x * 0.9).sin() + (i * 7 % 13) as f64);
        mem.push(55.0 + 10.0 * (x * 0.21).sin());
    }
    let skip = n.saturating_sub(600);
    let cols = vec![
        ("t".into(), Column::F64(t[skip..].to_vec())),
        ("cpu".into(), Column::F64(cpu[skip..].to_vec())),
        ("mem".into(), Column::F64(mem[skip..].to_vec())),
    ];
    match ch {
        Some(ch) => ch.publish(cols).unwrap(),
        None => Channel::table(cols).unwrap(),
    }
}

#[test]
fn a_live_line_charts_area_covers_what_its_line_covers_at_both_edges() {
    // As samples left the plot at the left, the area went before the line
    // and jumped back with each sample. The rows drawn started at the first
    // one at or past the window's edge, found by comparing floats: x
    // accumulated by 0.1 put the row meant for the edge a hair before it,
    // the search landed a row late, the slide uncovered a sample's width of
    // nothing at the plot's left edge, and the line's round cap stood past
    // its area's end there. Through several samples of the gallery's feed,
    // its frames built by the view and again by the core, at 1 and 1.5:
    // the line and the area under it are both cut by the plot's clip, at
    // both its edges, on every frame — the old samples leaving at the left
    // and the newest coming in at the right.
    let red = kui_core::Color::hex(0xff0000ff);
    for scale in [1.0f32, 1.5] {
        let mut r = rig();
        let mut n = 400;
        let mut ch = feed_samples(None, n);
        let frame = |r: &mut Rig, ch: &Channel, t: f64| {
            r.core.set_time(t);
            let mut ui = r.core.frame_with(VIEW, scale, &mut r.exts);
            ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(6.0));
            ui.with(
                NodeSpec::column().size(700.0, 300.0).pad_xy(7.3, 0.0),
                |ui| {
                    kit::LineChart::new(
                        ch.clone(),
                        "t",
                        vec![
                            kit::Series::new("cpu", "CPU").color(0xff0000ffu32),
                            kit::Series::new("mem", "Memory"),
                        ],
                    )
                    .window(30.0)
                    .area(true)
                    .place(ui);
                },
            );
            ui.finish();
            let ev = r.core.take_pending_events();
            r.exts.route(ev, |_| {});
        };
        let mut t = 1.0;
        // The pace learnt, and the y range settled.
        for _ in 0..12 {
            frame(&mut r, &ch, t);
            t += 0.1;
            n += 1;
            ch = feed_samples(Some(&ch), n);
        }
        let mut frames = 0;
        let mut slid = false;
        for _ in 0..8 {
            for k in 0..6 {
                let now = t + k as f64 * 0.016;
                r.core.set_time(now);
                // As the runner draws them: a view where a frame needs one,
                // the kept tree where only the slide moved.
                if k % 2 == 0 || !r.core.reframe(VIEW, scale) {
                    frame(&mut r, &ch, now);
                }
                let (dl, _) = r.core.output();
                let line: Vec<_> = dl
                    .quads
                    .iter()
                    .filter(|q| q.kind == kui_core::QuadKind::Segment && q.color == red)
                    .collect();
                let area: Vec<_> = dl
                    .quads
                    .iter()
                    .filter(|q| q.kind == kui_core::QuadKind::Fragment)
                    .collect();
                assert!(
                    line.len() > 50 && area.len() > 10,
                    "drawn: {} {}",
                    line.len(),
                    area.len()
                );
                let clip = dl.clip_of(line[0]).rect;
                assert!(
                    area.iter().all(|q| dl.clip_of(q).rect == clip),
                    "one clip, the plot's"
                );
                // Where each paints, inside the clip: the line with its
                // round caps, the area by its polygons' vertices.
                let cut = |lo: f32, hi: f32| (lo.max(clip.x), hi.min(clip.x + clip.w));
                let (mut l_lo, mut l_hi) = (f32::MAX, f32::MIN);
                for q in &line {
                    let e = q.segment_ends();
                    let half = q.border_w / 2.0;
                    l_lo = l_lo.min(e[0].min(e[2]) - half);
                    l_hi = l_hi.max(e[0].max(e[2]) + half);
                }
                let (mut a_lo, mut a_hi) = (f32::MAX, f32::MIN);
                for q in &area {
                    let p = dl.fragments[q.uv[0] as usize].params;
                    for v in 0..8 {
                        let x = q.rect.x + p[v * 2] * q.rect.w;
                        a_lo = a_lo.min(x);
                        a_hi = a_hi.max(x);
                    }
                }
                let (line_seen, area_seen) = (cut(l_lo, l_hi), cut(a_lo, a_hi));
                let at = format!("scale {scale}, at {now:.3}, frame {k}");
                assert!(
                    (line_seen.0 - area_seen.0).abs() < 0.01
                        && (line_seen.1 - area_seen.1).abs() < 0.01,
                    "{at}: the area covers what the line does: line {line_seen:?}, area {area_seen:?}"
                );
                assert!(
                    line_seen.0 <= clip.x + 0.01 && line_seen.1 >= clip.x + clip.w - 0.01,
                    "{at}: both cross the plot's edges, cut there: {line_seen:?} in {clip:?}"
                );
                slid |= area_seen.0 > a_lo + 0.5;
                frames += 1;
            }
            t += 0.1;
            n += 1;
            ch = feed_samples(Some(&ch), n);
        }
        assert_eq!(frames, 48);
        assert!(
            slid,
            "what is drawn reached past the left edge, the clip cutting it"
        );
        assert!(r.core.take_warnings().is_empty());
    }
}

#[test]
fn a_chart_is_a_tab_stop_only_when_the_app_asks() {
    // A line chart is a Tab stop, a slider over its samples, only with
    // `focusable = true`; a bar chart's bars only with `on_select`, bars to
    // pick. Every chart as Watchtower and the gallery place them — a live
    // line with its area and a window, a sparkline, a histogram, bars
    // standing and lying — is passed over, and the pointer's readout still
    // works on a line chart that is no stop.
    let mut r = LuaRig::new(
        r#"
        local t, a, n, names = {}, {}, {}, {}
        for i = 0, 29 do t[#t + 1] = i; a[#a + 1] = (i * 7) % 31; n[#n + 1] = i % 5 + 1; names[#names + 1] = "s" .. i end
        local ch = channel({ t = t, a = a, n = n, name = names })
        function view(env)
          return column { width = 900, height = "grow", pad = 20, gap = 8, scroll_y = true,
            component("kit/button", { label = "Before" }),
            row { key = "quiet", label = "quiet", width = "grow",
              component("kit/line_chart", { data = ch, x = "t", window = 20, area = true, height = 120,
                series = { { field = "a", name = "A" } } }) },
            component("kit/sparkline", { data = ch, field = "a", width = 160, height = 24 }),
            component("kit/histogram", { data = ch, field = "a", bins = 10, height = 80 }),
            component("kit/bar_chart", { data = ch, label = "name", value = "n", height = 80 }),
            component("kit/bar_chart", { data = ch, label = "name", value = "n", horizontal = true, label_width = 40 }),
            component("kit/line_chart", { data = ch, x = "t", height = 120, focusable = true,
              series = { { field = "a", name = "Z" } } }),
            component("kit/bar_chart", { data = ch, label = "name", value = "n", height = 80, on_select = { kind = "pick" } }),
            component("kit/button", { label = "After" }) }
        end
        "#,
    );
    r.frames(3);
    // The readout under the pointer, on the line chart that is no stop.
    let quiet = r
        .node(|n| n.label.as_deref() == Some("quiet"))
        .unwrap()
        .rect;
    use kui_core::{InputEvent, Vec2};
    let e = r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        quiet.x + quiet.w / 2.0,
        quiet.y + 50.0,
    )));
    r.route(e);
    r.frames(3);
    // (An image to a screen reader, its readout box is drawn, not read.)
    let said = r
        .node(|n| n.label.as_deref() == Some("readout"))
        .map(|n| n.rect);
    assert!(
        said.is_some_and(|b| b.y >= quiet.y && b.y + b.h <= quiet.y + quiet.h),
        "the pointer's readout, on the chart that is no stop: {said:?} in {quiet:?}"
    );
    let e = r
        .core
        .handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
    r.route(e);
    r.frames(2);
    let focused = |r: &mut LuaRig| {
        r.core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.focused)
            .map(|n| (n.name.clone().unwrap_or_default(), n.role))
    };
    let mut seen = Vec::new();
    for _ in 0..6 {
        let e = kui_core::testing::tab(&mut r.core, false);
        r.route(e);
        r.frames(1);
        seen.push(focused(&mut r).expect("a stop"));
    }
    let names: Vec<&str> = seen.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(names[0], "Before", "{seen:?}");
    assert_eq!(
        names[1], "line chart",
        "the line chart that asked: {seen:?}"
    );
    assert_eq!(seen[1].1, kui_core::Role::Slider, "{seen:?}");
    assert!(
        names[2].starts_with("s0"),
        "the bars to pick, on their first: {seen:?}"
    );
    assert_eq!(names[3], "After", "every other chart passed over: {seen:?}");
    assert_eq!(&names[4..], &names[..2], "and round again: {seen:?}");
    assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
}

/// A table of sixty rows of `t` and `cpu`, a wave between 10 and 70
/// scaled by `k`.
fn wave(ch: Option<&Channel>, k: f64) -> Channel {
    let n = 60;
    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let cpu: Vec<f64> = (0..n)
        .map(|i| k * (40.0 + 30.0 * (i as f64 * 0.3).sin()))
        .collect();
    let cols = vec![
        ("t".into(), Column::F64(t)),
        ("cpu".into(), Column::F64(cpu)),
    ];
    match ch {
        Some(ch) => ch.publish(cols).unwrap(),
        None => Channel::table(cols).unwrap(),
    }
}

#[test]
fn a_line_charts_range_change_glides_on_the_cores_clock_with_no_view_run_between() {
    // C149: a range that changed glided by asking for a frame every frame
    // for 240 ms, each running the view. The lines are drawn for the new
    // range at once, in a box that enters stretched and moved to where the
    // old range had them and settles on the core's clock: the frames
    // between are built from the kept tree (`Core::reframe`, as the
    // runner builds them) and nothing asks the view for one.
    let red = kui_core::Color::hex(0xff0000ff);
    let mut r = rig();
    let frame = |r: &mut Rig, ch: &Channel, t: f64| {
        r.core.set_time(t);
        let mut ui = r.core.frame_with(VIEW, 1.0, &mut r.exts);
        ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(6.0));
        ui.with(NodeSpec::column().size(700.0, 300.0), |ui| {
            kit::LineChart::new(
                ch.clone(),
                "t",
                vec![kit::Series::new("cpu", "CPU").color(0xff0000ffu32)],
            )
            .area(true)
            .place(ui);
        });
        ui.finish();
        let ev = r.core.take_pending_events();
        r.exts.route(ev, |_| {});
    };
    // The line's stretch down, as drawn: 1 at rest.
    let stretch = |r: &mut Rig| {
        let (dl, _) = r.core.output();
        let q = dl
            .quads
            .iter()
            .find(|q| q.kind == kui_core::QuadKind::Segment && q.color == red)
            .copied()
            .expect("the line");
        dl.clip_of(&q).transform.sy
    };
    let ch = wave(None, 1.0);
    // A driver that builds again asks from its first frame on; the first
    // ask is refused, and has a build keep what building again needs.
    assert!(!r.core.reframe(VIEW, 1.0));
    for t in [1.0, 1.1, 1.2] {
        frame(&mut r, &ch, t);
    }
    assert!(!r.core.animating(), "settled on 0..80: {:?}", r.core.owed());
    assert!((stretch(&mut r) - 1.0).abs() < 1e-4);
    // Half the wave: the range goes to 0..40, the gutter as wide as it
    // was (a gutter that eases moves the plot, which the view reads).
    let ch = wave(Some(&ch), 0.5);
    frame(&mut r, &ch, 2.0);
    let at_start = stretch(&mut r);
    assert!(
        (at_start - 0.5).abs() < 1e-3,
        "it starts where 0..80 drew it: {at_start}"
    );
    assert!(r.core.animating(), "the glide owes frames");
    assert_eq!(
        r.core.next_frame_at(),
        None,
        "nothing asks for a frame at a time"
    );
    // The frame after the change is the view's, as the runner draws it:
    // the build before it owed nothing, so kept nothing to build again
    // from (C138). From then on the core builds the glide's frames alone.
    r.core.set_time(2.04);
    if !r.core.reframe(VIEW, 1.0) {
        frame(&mut r, &ch, 2.04);
    }
    let mut last = stretch(&mut r);
    assert!(last > at_start && last < 1.0, "{last}");
    for t in [2.08, 2.12, 2.2] {
        r.core.set_time(t);
        assert!(
            r.core.reframe(VIEW, 1.0),
            "the frame at {t} is built from the kept tree, no view run"
        );
        assert_eq!(r.core.next_frame_at(), None);
        let now = stretch(&mut r);
        assert!(
            now > last && now < 1.0,
            "it settles: {last} then {now} at {t}"
        );
        if t == 2.12 {
            // Halfway in time, an ease-out cubic is seven eighths there.
            assert!((now - (0.5 + 0.5 * 0.875)).abs() < 1e-2, "{now}");
        }
        last = now;
    }
    r.core.set_time(2.3);
    assert!(r.core.reframe(VIEW, 1.0));
    assert!((stretch(&mut r) - 1.0).abs() < 1e-4);
    assert!(!r.core.animating(), "and owes nothing once there");
}
