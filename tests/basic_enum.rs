use mlua_gen::mlua_gen;

#[test]
pub fn test() {
    #[mlua_gen]
    pub(crate) enum Ab {
        Unit,
        Tuple(String, i32),
        Named { name: String, int: i32 },
    }

    let lua = mlua::Lua::new();
    lua.globals()
        .set("Test", Ab::Tuple(String::from("uwu"), 42))
        .unwrap();

    lua.load(include_str!("./basic_enum.lua")).exec().unwrap();
}
