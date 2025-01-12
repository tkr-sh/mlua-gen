use mlua_gen::mlua_gen;

#[test]
pub fn test() {
    #[mlua_gen(get = *, set = *, impl = [uwu(&self, i32, i32), owo(&mut self), new()])]
    struct Test {
        name: String,
        int: u32,
    }

    impl Test {
        fn uwu(&self, a: i32, b: i32) -> i32 {
            a + b
        }

        fn owo(&mut self) {
            self.int += 10;
        }

        fn new() -> Test {
            Test {
                name: String::from("new"),
                int: 0,
            }
        }
    }

    let lua = mlua::Lua::new();
    lua.globals()
        .set(
            "test",
            Test {
                name: String::from("name"),
                int: 32,
            },
        )
        .unwrap();

    lua.load(include_str!("./basic_struct.lua")).exec().unwrap();
}
