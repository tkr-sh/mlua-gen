use mlua_gen::mlua_gen;

#[test]
pub fn test() {
    #[mlua_gen(custom_fields = fields, custom_impls = impls)]
    struct Human {
        pub name: String,
        pub age: u8,
    }

    fn fields<T: ::mlua::UserDataFields<Human>>(fields: &mut T) {
        fields.add_field_method_get("name_age", |_, this| {
            Ok(format!("{} ({})", this.name, this.age))
        });
    }

    fn impls<M: ::mlua::UserDataMethods<Human>>(methods: &mut M) {
        methods.add_method("age_in_next_years", |_, this, years: u8| {
            Ok(this.age + years)
        });
    }

    let lua = mlua::Lua::new();
    lua.globals()
        .set(
            "Human",
            Human {
                name: String::from("name"),
                age: 32,
            },
        )
        .unwrap();

    lua.load(include_str!("./custom_struct.lua"))
        .exec()
        .unwrap();
}
