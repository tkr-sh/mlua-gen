use {
    mlua_gen::{LuaBuilder, mlua_gen},
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
};

#[derive(Debug, Clone)]
#[mlua_gen]
struct Inner {
    pub x: u32,
}

#[derive(Debug)]
#[mlua_gen]
struct Holder {
    pub map: HashMap<String, Inner>,
}

#[test]
pub fn test() -> mlua::Result<()> {
    let lua = mlua::Lua::new();
    Inner::to_globals(&lua)?;

    let mut map = HashMap::new();
    map.insert("a".into(), Inner { x: 1 });
    map.insert("b".into(), Inner { x: 2 });
    let h = Arc::new(Mutex::new(Holder { map }));
    lua.globals().set("h", h.clone())?;

    let h_peek = h.clone();
    lua.globals().set(
        "peek_x",
        lua.create_function(move |_, key: String| Ok(h_peek.lock().unwrap().map[&key].x))?,
    )?;

    lua.load(include_str!("./hashmap_of_mlua_gen.lua")).exec()?;
    Ok(())
}
