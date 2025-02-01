use mlua_gen::{mlua_gen, to_lua, LuaBuilder};

#[test]
pub fn test() -> mlua::Result<()> {
    #[mlua_gen]
    enum OptionalString {
        None,
        Some(String),
    }

    #[mlua_gen]
    struct Test {
        pub name: String,
    }

    #[mlua_gen]
    struct Unit;

    let lua = mlua::Lua::new();
    to_lua!(lua, Unit, Test, OptionalString);

    lua.load(include_str!("./to_lua_macro.lua")).exec().unwrap();

    Ok(())
}
