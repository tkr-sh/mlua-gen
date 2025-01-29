use mlua_gen::{mlua_gen, LuaBuilder};

#[test]
pub fn test() {
    #[mlua_gen(custom_fields = fields, custom_impls = impls)]
    #[derive(Default)]
    enum Animal {
        #[default]
        Pig,
        Dog(String),
        Cat {
            name: String,
            age:  u8,
        },
    }

    fn fields<T: ::mlua::UserDataFields<Animal>>(fields: &mut T) {
        fields.add_field_method_get("horse", |_, _this| Ok("No horse"));
    }

    fn impls<M: ::mlua::UserDataMethods<Animal>>(methods: &mut M) {
        methods.add_method("name", |_, this, _: ()| {
            Ok(match this {
                Animal::Pig => "Piggy".to_string(),
                Animal::Dog(name) => name.to_owned(),
                Animal::Cat { name, .. } => name.to_owned(),
            })
        });
    }

    let lua = mlua::Lua::new();
    Animal::to_globals(&lua).unwrap();

    lua.load(include_str!("./custom_enum.lua")).exec().unwrap();
}
