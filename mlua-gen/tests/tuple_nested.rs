//! Tuple struct: nested mlua_gen field and indexable collection field.

use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() -> mlua::Result<()> {
    #[derive(Debug, Clone)]
    #[mlua_gen]
    struct Inner {
        pub x: u32,
    }

    #[mlua_gen]
    struct Pair(pub Inner, pub Vec<u8>);

    let lua = mlua::Lua::new();
    Inner::to_globals(&lua)?;
    Pair::to_globals(&lua)?;

    lua.load(include_str!("./tuple_nested.lua")).exec()?;
    Ok(())
}
