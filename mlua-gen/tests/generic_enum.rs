use {
    mlua::{FromLua, IntoLua},
    mlua_gen::{LuaBuilder, mlua_gen},
};

#[test]
pub fn test() {
    #[mlua_gen]
    enum Optional<T: FromLua + Clone + IntoLua + Send + Sync + 'static> {
        None,
        Some(T),
    }

    let lua = mlua::Lua::new();
    Optional::<String>::to_globals(&lua).unwrap();
    Optional::<i32>::to_globals_as(&lua, "OptionalInt").unwrap();

    lua.load(include_str!("./generic_enum.lua")).exec().unwrap();
}
