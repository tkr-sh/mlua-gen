use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() -> mlua::Result<()> {
    #[mlua_gen(impl = [display_animal_name(&self, Animal)])]
    struct Test {
        pub name: String,
    }

    impl Test {
        fn display_animal_name(&self, animal: Animal) -> String {
            format!(
                "{name} {animal}",
                name = self.name,
                animal = match animal {
                    Animal::Cat => "gato",
                    Animal::Dog => "doggo",
                }
            )
        }
    }

    #[mlua_gen]
    #[derive(Debug, Clone, Copy)]
    pub enum Animal {
        Cat,
        Dog,
    }

    let lua = mlua::Lua::new();

    Animal::to_globals(&lua)?;
    Test::to_globals(&lua)?;

    lua.load(include_str!("./enum_as_arg.lua")).exec()?;

    Ok(())
}
