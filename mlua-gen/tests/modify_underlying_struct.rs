#![feature(specialization)]
use {
    mlua::{AnyUserData, FromLua, Function, IntoLua, Lua, Table, UserData, Value},
    mlua_gen::{LuaBuilder, mlua_gen},
    std::{
        ops::Index,
        sync::{Arc, Mutex},
    },
};

trait IsIndexable {
    fn is_indexable(&self) -> bool {
        false
    }
}

impl<T> IsIndexable for T {
    default fn is_indexable(&self) -> bool {
        false
    }
}

impl<T: Index<usize>> IsIndexable for T {
    fn is_indexable(&self) -> bool {
        true
    }
}

#[test]
pub fn test() -> mlua::Result<()> {
    #[derive(Debug)]
    struct VecWrapper {
        vec: Vec<u8>,
        other: OtherStruct,
    }

    #[derive(Debug)]
    struct OtherStruct {
        a: u8,
    }

    impl UserData for VecWrapper {
        fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
            // fields.add_field_method_get("vec", |_, this| Ok(this.vec.clone()));
            fields.add_field_function_get("vec", |lua: &Lua, this: AnyUserData| {
                let table = lua.create_table()?;
                let this_clone = this.clone();

                if this
                    .borrow::<Arc<Mutex<Self>>>()?
                    .lock()
                    .unwrap()
                    .vec
                    .is_indexable()
                {
                    let get = lua.create_function(move |_, (_, index): (Table, usize)| {
                        println!("{this:#?}");
                        let this = this.borrow::<Arc<Mutex<Self>>>()?;
                        Ok(this.lock().unwrap().vec.get(index).copied())
                    })?;
                    let set =
                        lua.create_function(move |_, (_, index, value): (Table, usize, u8)| {
                            println!("{this_clone:#?}");
                            let mut this = this_clone.borrow_mut::<Arc<Mutex<Self>>>()?;
                            if index >= this.lock().unwrap().vec.len() {
                                this.lock().unwrap().vec.resize(index + 1, 0);
                            }
                            this.lock().unwrap().vec[index] = value;
                            Ok(())
                        })?;

                    let mt = lua.create_table_from([("__index", get), ("__newindex", set)])?;
                    table.set_metatable(Some(mt));
                }

                Ok(table)
            });
        }
    }

    let lua = mlua::Lua::new();
    let vec_wrapper = Arc::new(Mutex::new(VecWrapper {
        vec: vec![0, 1],
        other: OtherStruct { a: 0 },
    }));

    lua.globals().set("vec_wrapper", vec_wrapper.clone())?;

    lua.load("print(vec_wrapper.vec[2])").exec()?;

    lua.load("vec_wrapper.vec[3] = 2").exec()?;

    assert_eq!(*vec_wrapper.lock().unwrap().vec.last().unwrap(), 2);

    Ok(())
}
