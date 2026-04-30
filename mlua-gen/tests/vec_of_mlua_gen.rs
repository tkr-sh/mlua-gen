use {
    mlua_gen::{LuaBuilder, mlua_gen},
    std::sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
#[mlua_gen]
struct Inner {
    pub x: u32,
}

#[derive(Debug)]
#[mlua_gen]
struct Holder {
    pub items: Vec<Inner>,
}

#[test]
pub fn test() -> mlua::Result<()> {
    let lua = mlua::Lua::new();
    Inner::to_globals(&lua)?;

    let h = Arc::new(Mutex::new(Holder {
        items: vec![Inner { x: 1 }, Inner { x: 2 }, Inner { x: 3 }],
    }));
    lua.globals().set("h", h.clone())?;

    let h_peek = h.clone();
    lua.globals().set(
        "peek_x",
        lua.create_function(move |_, one_based: usize| {
            Ok(h_peek.lock().unwrap().items[one_based - 1].x)
        })?,
    )?;

    lua.load(include_str!("./vec_of_mlua_gen.lua")).exec()?;
    Ok(())
}
