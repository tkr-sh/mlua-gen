use mlua_gen::{mlua_gen, LuaBuilder};

#[test]
fn test() {
    #[mlua_gen(impl = [default(), age(&self), set_age(&mut self, u8)])]
    #[derive(Default)]
    pub(crate) struct Human {
        pub(crate) name: String,
        age:             u8,
    }

    impl Human {
        pub(crate) fn age(&self) -> String {
            format!("{} years old", self.age)
        }

        pub(crate) fn set_age(&mut self, age: u8) {
            self.age = age;
        }
    }

    let lua = mlua::Lua::new();
    Human::to_globals(&lua).unwrap();

    lua.load(include_str!("./doc.lua")).exec().unwrap();
}
