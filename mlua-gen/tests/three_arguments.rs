use mlua_gen::{mlua_gen, LuaBuilder};

#[test]
pub fn test() {
    #[mlua_gen(impl = [triple_sum(&self, i32, u8, u64)])]
    struct Test(i32);

    impl Test {
        fn triple_sum(&self, a: i32, b: u8, c: u64) -> i32 {
            self.0 + a + i32::from(b) + i32::try_from(c).unwrap()
        }
    }

    let lua = mlua::Lua::new();
    Test::to_globals(&lua).unwrap();

    lua.load(include_str!("./three_arguments.lua"))
        .exec()
        .unwrap()
}
