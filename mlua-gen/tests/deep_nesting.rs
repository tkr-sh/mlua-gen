use {
    mlua_gen::mlua_gen,
    std::sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
#[mlua_gen]
struct C {
    pub v: u32,
}

#[derive(Debug, Clone)]
#[mlua_gen]
struct B {
    pub c: C,
}

#[derive(Debug, Clone)]
#[mlua_gen]
struct A {
    pub b: B,
}

#[test]
pub fn test() -> mlua::Result<()> {
    let lua = mlua::Lua::new();
    let a = Arc::new(Mutex::new(A {
        b: B { c: C { v: 0 } },
    }));
    lua.globals().set("a", a.clone())?;

    let a_peek = a.clone();
    lua.globals().set(
        "peek",
        lua.create_function(move |_, ()| Ok(a_peek.lock().unwrap().b.c.v))?,
    )?;

    lua.load(include_str!("./deep_nesting.lua")).exec()?;
    Ok(())
}
