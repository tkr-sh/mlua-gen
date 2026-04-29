//! Negative test: when `on_set` is not specified the macro must produce code
//! byte-identical to the pre-feature output, i.e. no hook is ever called.
//!
//! We register a sentinel free function that would tick a counter if codegen
//! accidentally reached for it, then assert the counter stays at 0 after a
//! Lua-side write.

use {
    mlua_gen::{mlua_gen, LuaBuilder},
    std::sync::atomic::{AtomicUsize, Ordering},
};

static SENTINEL: AtomicUsize = AtomicUsize::new(0);

#[allow(
    dead_code,
    reason = "never referenced from generated code; that's the point"
)]
fn would_have_fired() {
    SENTINEL.fetch_add(1, Ordering::Relaxed);
}

#[mlua_gen(get = *, set = *)]
struct Plain {
    value: u32,
}

#[test]
pub fn test() {
    let lua = mlua::Lua::new();
    Plain::to_globals(&lua).unwrap();

    lua.globals()
        .set(
            "sentinel",
            lua.create_function(|_, ()| Ok(SENTINEL.load(Ordering::Relaxed)))
                .unwrap(),
        )
        .unwrap();

    lua.load(include_str!("./on_set_struct_absent.lua"))
        .exec()
        .unwrap();
}
