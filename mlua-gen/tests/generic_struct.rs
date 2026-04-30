use {
    mlua::{FromLua, IntoLua},
    mlua_gen::{LuaBuilder, mlua_gen},
};

#[test]
pub fn test() {
    #[mlua_gen]
    struct Test<T: FromLua + Clone + IntoLua + Send + Sync + 'static> {
        pub name: T,
    }

    let lua = mlua::Lua::new();
    Test::<String>::to_globals(&lua).unwrap();
    Test::<i32>::to_globals_as(&lua, "TestInt").unwrap();

    lua.load(include_str!("./generic_struct.lua"))
        .exec()
        .unwrap();
}
