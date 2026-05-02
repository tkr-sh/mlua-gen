use mlua_gen::{LuaBuilder, mlua_gen};

#[mlua_gen]
#[allow(dead_code)] // variants are reachable through Lua, not Rust
enum Animal {
    Pig,
    Dog(String, u8),
}

#[test]
fn test() {
    let lua = mlua::Lua::new();
    Animal::to_globals(&lua).unwrap();

    lua.globals()
        .set("animal", Animal::Dog("rex".to_owned(), 3))
        .unwrap();

    lua.load(include_str!("./enum_in_place_unnamed.lua"))
        .exec()
        .unwrap();
}
