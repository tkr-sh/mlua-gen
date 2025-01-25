use mlua_gen::mlua_gen;

#[test]
pub fn test() {
    #[mlua_gen(get = *, set = *)]
    struct Unnamed(String, u32);

    let lua = mlua::Lua::new();
    lua.globals()
        .set("test", Unnamed(String::from("name"), 32))
        .unwrap();

    lua.load(include_str!("./unnamed_struct.lua"))
        .exec()
        .unwrap();
}
