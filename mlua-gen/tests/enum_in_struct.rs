use mlua_gen::{LuaBuilder, mlua_gen};

#[mlua_gen]
#[derive(Clone)]
#[allow(dead_code)] // variants are reachable through Lua, not Rust
enum Inner {
    Idle,
    Active { x: u32 },
    Tup(u32, u32),
}

#[mlua_gen(get = *, set = *)]
struct Outer {
    pub inner: Inner,
}

#[test]
fn test() {
    let lua = mlua::Lua::new();
    Inner::to_globals(&lua).unwrap();
    Outer::to_globals(&lua).unwrap();

    lua.globals()
        .set(
            "o",
            Outer {
                inner: Inner::Active { x: 1 },
            },
        )
        .unwrap();

    lua.load(include_str!("./enum_in_struct.lua"))
        .exec()
        .unwrap();
}
