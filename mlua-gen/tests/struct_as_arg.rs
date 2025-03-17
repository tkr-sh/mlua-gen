use mlua_gen::{LuaBuilder, mlua_gen};

#[test]
pub fn test() -> mlua::Result<()> {
    #[mlua_gen(impl = [display_qt_name(&self, Quantity)])]
    struct Test {
        pub name: String,
    }

    impl Test {
        fn display_qt_name(&self, qt: Quantity) -> String {
            format!("{qt} {name}", name = self.name, qt = qt.0,)
        }
    }

    #[mlua_gen]
    #[derive(Debug, Clone, Copy)]
    pub struct Quantity(usize);

    let lua = mlua::Lua::new();

    Quantity::to_globals(&lua)?;
    Test::to_globals(&lua)?;

    lua.load(include_str!("./struct_as_arg.lua")).exec()?;

    Ok(())
}
