#[path = "../src/playground.rs"]
mod playground;

#[test]
fn bounded_code_can_run_again_after_an_endless_loop() {
    let lua = mlua::Lua::new();
    playground::install(&lua).unwrap();
    let error = lua.load("while true do end").exec().unwrap_err();
    assert!(error.to_string().contains("instruction limit"));
    playground::reset();
    assert_eq!(lua.load("return 6 * 7").eval::<i32>().unwrap(), 42);
}

#[test]
fn coroutines_inherit_the_instruction_limit() {
    let lua = mlua::Lua::new();
    playground::reset();
    playground::install(&lua).unwrap();
    let (ok, message) = lua
        .load("local co = coroutine.create(function() while true do end end); local ok, err = coroutine.resume(co); return ok, tostring(err)")
        .eval::<(bool, String)>()
        .unwrap();
    assert!(!ok);
    assert!(message.contains("instruction limit"));
}

#[test]
fn excessive_lua_allocation_is_rejected() {
    let lua = mlua::Lua::new();
    playground::reset();
    playground::install(&lua).unwrap();
    assert!(
        lua.load("return string.rep('x', 64 * 1024 * 1024)")
            .eval::<mlua::LuaString>()
            .is_err()
    );
}
