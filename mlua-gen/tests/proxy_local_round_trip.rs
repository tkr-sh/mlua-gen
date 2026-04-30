use {
    mlua_gen::mlua_gen,
    std::sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
#[mlua_gen]
struct Inner {
    pub x: u32,
}

#[derive(Debug)]
#[mlua_gen]
struct Outer {
    pub inner: Inner,
}

#[test]
pub fn test() -> mlua::Result<()> {
    let lua = mlua::Lua::new();
    let o = Arc::new(Mutex::new(Outer {
        inner: Inner { x: 1 },
    }));
    lua.globals().set("o", o.clone())?;

    let o_peek = o.clone();
    lua.globals().set(
        "peek",
        lua.create_function(move |_, ()| Ok(o_peek.lock().unwrap().inner.x))?,
    )?;

    lua.load(include_str!("./proxy_local_round_trip.lua"))
        .exec()?;
    Ok(())
}
