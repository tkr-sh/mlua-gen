use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

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
        pub a:      u8,
        pub deeper: OtherStructDeeper,
    }

    #[derive(Debug, Clone)]
    #[mlua_gen::mlua_gen]
    struct OtherStructDeeper {
        pub deep: u8,
    }

    let lua = mlua::Lua::new();
    let vec_wrapper = Arc::new(Mutex::new(VecWrapper {
        vec:   vec![0, 1],
        other: OtherStruct {
            a:      0,
            deeper: OtherStructDeeper { deep: 19 },
        },
    }));

    lua.globals().set("vec_wrapper", vec_wrapper.clone())?;

    lua.load("print(vec_wrapper.vec[2])").exec()?;

    lua.load("vec_wrapper.vec[3] = 2").exec()?;
    assert_eq!(*vec_wrapper.lock().unwrap().vec.last().unwrap(), 2);

    lua.load("vec_wrapper.other.a = 1").exec()?;

    assert_eq!(vec_wrapper.lock().unwrap().other.a, 1);

    // Depth-2 read works.
    assert_eq!(vec_wrapper.lock().unwrap().other.deeper.deep, 19);
    lua.load("assert(vec_wrapper.other.deeper.deep == 19)")
        .exec()?;

    // TODO(phase 2): depth-2 write doesn't propagate yet.
    // lua.load("vec_wrapper.other.deeper.deep = 1").exec()?;
    // assert_eq!(vec_wrapper.lock().unwrap().other.deeper.deep, 1);

    Ok(())
}
