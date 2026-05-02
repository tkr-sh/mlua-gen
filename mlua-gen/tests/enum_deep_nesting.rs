//! 5+ proxies deep: struct → struct → enum router → variant → struct → leaf.

use mlua_gen::{LuaBuilder, mlua_gen};

#[mlua_gen(get = *, set = *)]
#[derive(Clone)]
struct Leaf {
    pub v: u32,
}

#[mlua_gen]
#[derive(Clone)]
#[allow(dead_code)] // variants are reachable through Lua, not Rust
enum Mid {
    Idle,
    Active { leaf: Leaf },
    Pair(Leaf, Leaf),
}

#[mlua_gen(get = *, set = *)]
#[derive(Clone)]
struct B {
    pub mid: Mid,
}

#[mlua_gen(get = *, set = *)]
struct A {
    pub b: B,
}

#[test]
fn test() {
    let lua = mlua::Lua::new();
    Leaf::to_globals(&lua).unwrap();
    Mid::to_globals(&lua).unwrap();
    B::to_globals(&lua).unwrap();
    A::to_globals(&lua).unwrap();

    lua.globals()
        .set(
            "a",
            A {
                b: B {
                    mid: Mid::Active {
                        leaf: Leaf { v: 1 },
                    },
                },
            },
        )
        .unwrap();

    lua.load(include_str!("./enum_deep_nesting.lua"))
        .exec()
        .unwrap();
}
