//! Fixed fragments and application sources share handles and module assembly;
//! only the fixed sources may skip runtime parsing and validation.

use kui_core::{Color, Core, NodeSpec, Session, Size, Span, TextStyle, Vec2, fragment, testing};

const GRADIENT: &str = r#"
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    return mix(params[0], params[1], in.local.y / max(in.size.y, 1.0));
}
"#;

fn polygon(core: &mut Core) -> kui_core::resources::FragmentId {
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.polygon(
        &[
            Vec2::new(10.0, 10.0),
            Vec2::new(80.0, 10.0),
            Vec2::new(40.0, 80.0),
        ],
        NodeSpec::column().bg(Color::WHITE),
    );
    ui.finish();
    let fragments = &core.output().0.fragments;
    assert_eq!(fragments.len(), 1);
    fragments[0].id
}

#[test]
fn application_shaders_still_reject_syntax_and_validation_errors() {
    for source in [
        "fn fragment(",
        // Parses, but returns the wrong type: Naga's validator must still run.
        "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> { return vec3<f32>(1.0); }",
    ] {
        let mut core = Core::new();
        polygon(&mut core);
        assert!(core.add_fragment(source).is_none());
        assert!(testing::codes(&core.take_warnings()).contains(&"fragment-rejected"));
    }
}

#[test]
fn application_shaders_remain_idempotent_and_can_be_registered_after_removal() {
    let mut core = Core::new();
    let id = core.add_fragment(GRADIENT).expect("valid application WGSL");
    assert_eq!(core.add_fragment(GRADIENT), Some(id));
    assert_eq!(
        core.fragment_module_source(id),
        Some(fragment::module_source(GRADIENT))
    );
    core.remove_fragment(id);
    let next = core.add_fragment(GRADIENT).unwrap();
    assert_ne!(id, next);
    assert_eq!(core.add_fragment(GRADIENT), Some(next));
}

#[test]
fn polygons_share_the_session_registry_and_recover_after_removal() {
    let session = Session::new();
    let mut a = Core::new_in(&session);
    let mut b = Core::new_in(&session);
    let id = polygon(&mut a);
    assert_eq!(polygon(&mut a), id);
    assert_eq!(polygon(&mut b), id);
    assert_eq!(a.add_fragment(fragment::POLYGON), Some(id));
    assert_eq!(
        a.fragment_module_source(id),
        Some(fragment::module_source(fragment::POLYGON))
    );
    a.remove_fragment(id);
    let next = polygon(&mut a);
    assert_ne!(next, id);
    assert_eq!(a.add_fragment(fragment::POLYGON), Some(next));
}

#[test]
fn an_application_registered_polygon_is_reused_by_the_builtin() {
    let mut core = Core::new();
    let id = core.add_fragment(fragment::POLYGON).unwrap();
    assert_eq!(polygon(&mut core), id);
    // Starting with a built-in's text does not make an edited source trusted.
    let invalid = format!("{}\nnot valid WGSL", fragment::POLYGON);
    assert!(core.add_fragment(&invalid).is_none());
    assert!(testing::codes(&core.take_warnings()).contains(&"fragment-rejected"));
}

#[test]
fn joined_backgrounds_reuse_the_validated_module_and_recover_after_removal() {
    fn joined(core: &mut Core) -> kui_core::resources::FragmentId {
        let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.rich_text(
            &[Span::new("joined background")
                .bg(Color::WHITE)
                .bg_radius(4.0)],
            TextStyle::new(14.0),
        );
        ui.finish();
        let fragments = &core.output().0.fragments;
        assert_eq!(fragments.len(), 1);
        fragments[0].id
    }

    let mut core = Core::new();
    let id = joined(&mut core);
    assert_eq!(joined(&mut core), id);
    assert_eq!(core.add_fragment(fragment::JOIN), Some(id));
    assert_eq!(
        core.fragment_module_source(id),
        Some(fragment::module_source(fragment::JOIN))
    );
    core.remove_fragment(id);
    let next = joined(&mut core);
    assert_ne!(next, id);
    assert_eq!(core.add_fragment(fragment::JOIN), Some(next));
}
