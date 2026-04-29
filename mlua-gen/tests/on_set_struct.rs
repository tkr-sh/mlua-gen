use {
    mlua_gen::{LuaBuilder, mlua_gen},
    std::sync::atomic::{AtomicUsize, Ordering},
};

static HITS: AtomicUsize = AtomicUsize::new(0);

fn on_set_hook() {
    HITS.fetch_add(1, Ordering::Relaxed);
}

#[mlua_gen(get = *, set = *, on_set = crate::on_set_hook)]
struct App {
    focused_buffer: u32,
    title:          String,
}

#[test]
pub fn test() {
    let lua = mlua::Lua::new();
    App::to_globals(&lua).unwrap();

    lua.globals()
        .set(
            "hits",
            lua.create_function(|_, ()| Ok(HITS.load(Ordering::Relaxed)))
                .unwrap(),
        )
        .unwrap();

    lua.load(include_str!("./on_set_struct.lua"))
        .exec()
        .unwrap();
}
