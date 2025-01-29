use mlua_gen::{mlua_gen, LuaBuilder};

#[test]
fn test() {
    #[mlua_gen]
    enum Animal {
        Pig,
        Dog(String, u8),
        Cat { name: String, age: u8 },
    }

    let lua = mlua::Lua::new();
    Animal::to_globals(&lua).unwrap();

    lua.load(include_str!("./basic_enum.lua")).exec().unwrap();
}
