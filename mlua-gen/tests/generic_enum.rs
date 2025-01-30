use {
    mlua::{FromLua, IntoLua},
    mlua_gen::{mlua_gen, LuaBuilder},
};

#[test]
pub fn test() {
    #[mlua_gen]
    enum Optional<T: FromLua + Clone + IntoLua + 'static> {
        None,
        Some(T),
    }

    let lua = mlua::Lua::new();
    Optional::<String>::to_globals(&lua).unwrap();
    Optional::<i32>::to_globals_as(&lua, "OptionalInt").unwrap();

    lua.load(include_str!("./generic_enum.lua")).exec().unwrap()
}
