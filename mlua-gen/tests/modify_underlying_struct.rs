use std::sync::{Arc, Mutex};

#[test]
pub fn test() -> mlua::Result<()> {
    #[derive(Debug)]
    #[mlua_gen::mlua_gen]
    struct VecWrapper {
        pub vec:   Vec<u8>,
        pub other: OtherStruct,
    }

    #[derive(Debug, Clone)]
    #[mlua_gen::mlua_gen]
    struct OtherStruct {
        pub a: u8,
    }



    // impl UserData for VecWrapper {
    //     fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
    //         // fields.add_field_method_get("vec", |_, this| Ok(this.vec.clone()));
    //         fields.add_field_function_get("vec", |lua: &Lua, this: AnyUserData| {
    //             let table = lua.create_table()?;
    //             let this_clone = this.clone();
    //
    //             if <Vec<u8> as IsIndexable>::IS_INDEXABLE {
    //                 let get = lua.create_function(move |_, (_, index): (Table, usize)| {
    //                     println!("{this:#?}");
    //                     let this = this.borrow::<Arc<Mutex<Self>>>()?;
    //                     Ok(this.lock().unwrap().vec.get(index).copied())
    //                 })?;
    //                 let set =
    //                     lua.create_function(move |_, (_, index, value): (Table, usize, u8)| {
    //                         println!("{this_clone:#?}");
    //                         let mut this = this_clone.borrow_mut::<Arc<Mutex<Self>>>()?;
    //                         if index >= this.lock().unwrap().vec.len() {
    //                             this.lock().unwrap().vec.resize(index + 1, 0);
    //                         }
    //                         this.lock().unwrap().vec[index] = value;
    //                         Ok(())
    //                     })?;
    //
    //                 let mt = lua.create_table_from([("__index", get), ("__newindex", set)])?;
    //                 table.set_metatable(Some(mt));
    //             }
    //
    //             Ok(table)
    //         });
    //
    //         fields.add_field_function_get("other", |lua: &Lua, this: AnyUserData| {
    //             let table = lua.create_table()?;
    //             let this_clone = this.clone();
    //
    //             if <OtherStruct as IsMluaGenerated>::IS_MLUA_GENERATED {
    //                 let get = lua.create_function(move |_, (_, index): (Table, usize)| {
    //                     println!("{this:#?}");
    //                     let this = this.borrow::<Arc<Mutex<Self>>>()?;
    //                     Ok(this.lock().unwrap().other.clone())
    //                 })?;
    //                 let set = lua.create_function(
    //                     move |lua, (_, key, value): (Table, String, Value)| {
    //                         println!("{this_clone:#?}");
    //                         let mut this = this_clone.borrow_mut::<Arc<Mutex<Self>>>()?;
    //                         let Value::UserData(uwu) = this.lock().unwrap().other.clone().into_lua(lua)? else {
    //                             unreachable!("Conversion of types that come from mlua-gen should ALWAYS be converted to table")
    //                         };
    //
    //                         use ::mlua::ObjectLike;
    //                         uwu.set(key.clone(), value);
    //
    //                         this.lock().unwrap().other = FromLua::from_lua(Value::UserData(uwu), lua)?;
    //                         Ok(())
    //                     },
    //                 )?;
    //
    //                 let mt = lua.create_table_from([("__index", get), ("__newindex", set)])?;
    //                 table.set_metatable(Some(mt));
    //             }
    //
    //             Ok(table)
    //         });
    //     }
    // }

    let lua = mlua::Lua::new();
    let vec_wrapper = Arc::new(Mutex::new(VecWrapper {
        vec:   vec![0, 1],
        other: OtherStruct { a: 0 },
    }));

    lua.globals().set("vec_wrapper", vec_wrapper.clone())?;

    lua.load("print(vec_wrapper.vec[2])").exec()?;

    lua.load("vec_wrapper.vec[3] = 2").exec()?;
    assert_eq!(*vec_wrapper.lock().unwrap().vec.last().unwrap(), 2);

    lua.load("vec_wrapper.other.a = 1").exec()?;

    assert_eq!(vec_wrapper.lock().unwrap().other.a, 1);

    Ok(())
}
