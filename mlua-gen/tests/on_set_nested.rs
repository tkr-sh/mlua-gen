use {
    mlua_gen::{LuaBuilder, mlua_gen},
    std::sync::{
        Arc,
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

static HITS: AtomicUsize = AtomicUsize::new(0);

fn on_set_hook() {
    HITS.fetch_add(1, Ordering::Relaxed);
}

#[derive(Debug, Clone)]
#[mlua_gen]
struct Leaf {
    pub v: u32,
}

#[derive(Debug, Clone)]
#[mlua_gen]
struct Mid {
    pub leaf: Leaf,
}

#[derive(Debug)]
#[mlua_gen(get = *, set = *, on_set = crate::on_set_hook)]
struct Root {
    pub mid:   Mid,
    pub items: Vec<Leaf>,
}

#[test]
pub fn test() -> mlua::Result<()> {
    HITS.store(0, Ordering::Relaxed);

    let lua = mlua::Lua::new();
    Leaf::to_globals(&lua)?;
    let r = Arc::new(Mutex::new(Root {
        mid:   Mid {
            leaf: Leaf { v: 0 },
        },
        items: vec![Leaf { v: 10 }, Leaf { v: 20 }],
    }));
    lua.globals().set("r", r.clone())?;

    lua.globals().set(
        "hits",
        lua.create_function(|_, ()| Ok(HITS.load(Ordering::Relaxed)))?,
    )?;

    let r_leaf = r.clone();
    lua.globals().set(
        "peek_leaf",
        lua.create_function(move |_, ()| Ok(r_leaf.lock().unwrap().mid.leaf.v))?,
    )?;

    let r_item = r.clone();
    lua.globals().set(
        "peek_item",
        lua.create_function(move |_, one_based: usize| {
            Ok(r_item.lock().unwrap().items[one_based - 1].v)
        })?,
    )?;

    lua.load(include_str!("./on_set_nested.lua")).exec()?;
    Ok(())
}
