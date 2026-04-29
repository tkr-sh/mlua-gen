//! Lua-constructed parent supports depth-1 nested writes.

use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() -> mlua::Result<()> {
    #[derive(Debug, Clone)]
    #[mlua_gen]
    struct Inner {
        pub x: u32,
    }

    #[mlua_gen]
    struct Outer {
        pub inner: Inner,
    }

    let lua = mlua::Lua::new();
    Outer::to_globals(&lua)?;
    Inner::to_globals(&lua)?;

    lua.load(include_str!("./lua_constructed_nested.lua"))
        .exec()?;
    Ok(())
}
