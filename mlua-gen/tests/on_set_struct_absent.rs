use {
    mlua_gen::{LuaBuilder, mlua_gen},
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
