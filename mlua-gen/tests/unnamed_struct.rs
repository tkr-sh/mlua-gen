use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() {
    #[mlua_gen(get = *, set = *)]
    struct Unnamed(String, u32);

    let lua = mlua::Lua::new();
    Unnamed::to_globals(&lua).unwrap();

    lua.load(include_str!("./unnamed_struct.lua"))
        .exec()
        .unwrap();
}
