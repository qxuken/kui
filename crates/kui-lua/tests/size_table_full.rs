//! A Lua view past the full size-expression table (backlog RG93). The
//! table is the process's, so this binary is its own.

use kui_core::calc::{self, Expr};
use kui_core::{Core, Extension, OriginId, Size, Slot, diag};
use kui_lua::LuaExtension;

/// A new expression in a view once the table is full leaves that prop at
/// its default — the width fit, the clamp none — and the frame builds:
/// before, `bad()` failed the view whole.
#[test]
fn a_new_expression_past_the_full_table_leaves_its_prop_undeclared() {
    let mut i = 0;
    while calc::intern(Expr::Max(vec![Expr::Px(i as f32 + 0.5), Expr::Pct(0.5)])).is_ok() {
        i += 1;
    }
    let mut ext = LuaExtension::from_source(
        "full",
        r#"
            function view(env)
              return column { width = 400, gap = 0,
                row { key = "kept", width = "max(0.5px, 50%)", height = 4 },
                row { key = "spelled", width = "clamp(1px, 50%, 300px)", height = 4 },
                row { key = "data", width = { min = { 7, { pct = 50 } } }, height = 4 },
                row { key = "capped", width = 350, max_width = "min(3px, 50%)", height = 4 },
                row { key = "floored", width = 10, min_width = { max = { 11, { pct = 50 } } },
                      height = 4 },
              }
            end
        "#,
    )
    .unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    let mut ui = core.frame(Size::new(600.0, 200.0), 1.0);
    ui.set_origin(OriginId(1));
    ext.view(&Slot::root(), &mut ui).expect("the frame builds");
    ui.finish();
    let nodes = core.nodes();
    let w: Vec<f32> = ["kept", "spelled", "data", "capped", "floored"]
        .map(|l| nodes.iter().find(|n| n.label.as_deref() == Some(l)).expect(l).rect.w)
        .to_vec();
    assert_eq!(w, [200.0, 0.0, 0.0, 350.0, 10.0]);
    let full = core
        .take_warnings()
        .into_iter()
        .filter(|w| w.code == diag::SIZE_EXPRESSIONS_FULL)
        .count();
    assert_eq!(full, 1);
}
