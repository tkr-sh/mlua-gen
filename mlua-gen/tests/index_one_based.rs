//! Collection index 0 from Lua errors cleanly (used to underflow `usize`).

use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() -> mlua::Result<()> {
    #[mlua_gen]
    struct Holder {
        pub xs: Vec<u8>,
    }

    let lua = mlua::Lua::new();
    Holder::to_globals(&lua)?;

    lua.load(include_str!("./index_one_based.lua")).exec()?;
    Ok(())
}
