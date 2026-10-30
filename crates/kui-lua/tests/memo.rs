//! `memo(key, deps, function() … end)` from Lua (backlog F285): a part of
//! a script's view built once and pushed again while its deps hold, in a
//! view drawn fresh every frame - the script's function not called at all.

use kui_core::{Core, Extensions, NodeSpec, Size, Value};
use kui_lua::LuaExtension;

const SCRIPT: &str = r#"
calls = 0
components = {
  screen = function(env, props)
    return column {
      text("calls " .. calls),
      memo("units", { n = props.n }, function()
        calls = calls + 1
        if props.fail then error("boom") end
        local t = {}
        for i = 1, props.n do t[#t + 1] = text("unit " .. i) end
        return t
      end),
      memo(7, props.n, function() return text("seven") end),
    }
  end,
}
"#;

struct Rig {
    core: Core,
    exts: Extensions,
}

impl Rig {
    fn new() -> Self {
        let mut exts = Extensions::new();
        exts.push_as(
            "s",
            Box::new(LuaExtension::from_source("s.lua", SCRIPT).unwrap()),
        )
        .unwrap();
        Rig {
            core: Core::new(),
            exts,
        }
    }

    /// One frame of the screen, drawn fresh (as an app's own view is);
    /// the texts it drew.
    fn frame(&mut self, n: i64, fail: bool) -> Vec<String> {
        let props = Value::map([
            ("n", Value::Int(n)),
            ("fail", Value::Bool(fail)),
            ("fresh", Value::Bool(true)),
        ]);
        let mut ui = self
            .core
            .frame_with(Size::new(400.0, 600.0), 1.0, &mut self.exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.component("s/screen", &props);
        ui.finish();
        self.core
            .access_tree()
            .nodes
            .iter()
            .filter_map(|n| n.name.clone())
            .collect()
    }
}

fn has(texts: &[String], t: &str) -> bool {
    texts.iter().any(|x| x == t)
}

#[test]
fn a_memo_in_a_script_runs_its_function_only_when_its_deps_move() {
    let mut r = Rig::new();
    let t = r.frame(3, false);
    assert!(
        has(&t, "calls 0") && has(&t, "unit 3") && has(&t, "seven"),
        "{t:?}"
    );
    // The view ran again; the memo's function did not.
    let t = r.frame(3, false);
    assert!(
        has(&t, "calls 1") && has(&t, "unit 3") && has(&t, "seven"),
        "{t:?}"
    );
    let t = r.frame(3, false);
    assert!(has(&t, "calls 1") && !has(&t, "unit 4"), "{t:?}");
    // Its deps moved: called once more.
    let t = r.frame(4, false);
    assert!(has(&t, "calls 1") && has(&t, "unit 4"), "{t:?}");
    let t = r.frame(4, false);
    assert!(has(&t, "calls 2") && has(&t, "unit 4"), "{t:?}");
}

#[test]
fn a_memo_whose_function_raised_keeps_nothing() {
    let mut r = Rig::new();
    r.frame(5, false);
    // Raises with new deps: nothing of it is kept.
    r.frame(6, true);
    // The same deps without the error: called again, not replayed half.
    let t = r.frame(6, false);
    assert!(has(&t, "calls 2"), "{t:?}");
    let t = r.frame(6, false);
    assert!(has(&t, "calls 3") && has(&t, "unit 6"), "{t:?}");
}
