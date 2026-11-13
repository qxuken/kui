//! Limits for editable browser examples. Normal packaged apps do not install
//! these hooks. The preview resets the allowance before each Wasm entry call.
use std::cell::Cell;

use mlua::{HookTriggers, Lua, VmState};

const INSTRUCTIONS: usize = 2_000_000;
const INTERVAL: u32 = 10_000;

thread_local! {
    static REMAINING: Cell<usize> = const { Cell::new(INSTRUCTIONS) };
}

pub(crate) fn reset() {
    REMAINING.set(INSTRUCTIONS);
}

pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    lua.set_memory_limit(32 * 1024 * 1024)?;
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(INTERVAL),
        |_, _| {
            REMAINING.with(|remaining| {
                let next = remaining.get().saturating_sub(INTERVAL as usize);
                remaining.set(next);
                if next == 0 {
                    Err(mlua::Error::runtime(
                        "Playground instruction limit reached. Check for an endless loop.",
                    ))
                } else {
                    Ok(VmState::Continue)
                }
            })
        },
    )
}
