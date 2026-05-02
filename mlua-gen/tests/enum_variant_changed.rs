use mlua_gen::{LuaBuilder, mlua_gen};

#[mlua_gen]
#[allow(dead_code)] // variants are reachable through Lua, not Rust
enum State {
    Idle,
    Active { x: u32 },
    Other(u32),
}

#[test]
fn test() {
    let lua = mlua::Lua::new();
    State::to_globals(&lua).unwrap();

    lua.globals().set("state", State::Active { x: 1 }).unwrap();

    lua.load(include_str!("./enum_variant_changed.lua"))
        .exec()
        .unwrap();
}
