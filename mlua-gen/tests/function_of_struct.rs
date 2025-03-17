use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() {
    #[derive(Default)]
    #[mlua_gen(impl = [default(), new_martin()])]
    struct Human(pub(crate) String);

    impl Human {
        fn new_martin() -> Self {
            Human(String::from("Martin"))
        }
    }

    let lua = mlua::Lua::new();
    Human::to_globals(&lua).unwrap();

    lua.load(include_str!("./function_of_struct.lua"))
        .exec()
        .unwrap();
}
