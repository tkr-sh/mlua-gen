use {
    mlua::{FromLua, IntoLua},
    mlua_gen::{mlua_gen, LuaBuilder},
};

#[test]
pub fn test() {
    #[mlua_gen]
    struct Test<T: FromLua + Clone + IntoLua + 'static> {
        pub name: T,
    }

    let lua = mlua::Lua::new();
    Test::<String>::to_globals(&lua).unwrap();
    Test::<i32>::to_globals_as(&lua, "TestInt").unwrap();

    lua.load(include_str!("./generic_struct.lua"))
        .exec()
        .unwrap();
}
