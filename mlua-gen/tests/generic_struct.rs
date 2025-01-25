use {
    mlua::{FromLua, IntoLua},
    mlua_gen::mlua_gen,
};

#[test]
pub fn test() {
    #[mlua_gen]
    struct Test<T: FromLua + Clone + IntoLua> {
        pub name: T,
    }

    let lua = mlua::Lua::new();
    lua.globals()
        .set(
            "test",
            Test {
                name: String::from("name"),
            },
        )
        .unwrap();

    assert!(lua
        .load(include_str!("./generic_struct.lua"))
        .exec()
        .is_ok())
}
