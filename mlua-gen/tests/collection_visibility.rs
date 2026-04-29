//! `get` / `set` visibility applies to indexable-collection field proxies.

use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() -> mlua::Result<()> {
    // `xs` is readable but not writable from Lua: only `[ys]` is in `set`.
    #[mlua_gen(get = *, set = [ys])]
    struct Holder {
        pub xs: Vec<u8>,
        pub ys: Vec<u8>,
    }

    let lua = mlua::Lua::new();
    Holder::to_globals(&lua)?;

    lua.load(include_str!("./collection_visibility.lua"))
        .exec()?;
    Ok(())
}
