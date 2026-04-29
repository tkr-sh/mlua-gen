//! `on_set = <path>` on an enum: the hook fires once per Lua-side variant
//! swap, for both unnamed and named variants.

use {
    mlua_gen::{LuaBuilder, mlua_gen},
    std::sync::atomic::{AtomicUsize, Ordering},
};

static HITS: AtomicUsize = AtomicUsize::new(0);

fn on_set_hook() {
    HITS.fetch_add(1, Ordering::Relaxed);
}

#[mlua_gen(on_set = crate::on_set_hook)]
#[allow(dead_code)] // variants are reachable through Lua, not Rust
enum State {
    Idle,
    Active(u32),
    Labelled { name: String, count: u32 },
}

#[test]
pub fn test() {
    let lua = mlua::Lua::new();
    State::to_globals(&lua).unwrap();

    // Seed with an Idle value so the .lua side can mutate it through the
    // generated variant setters.
    lua.globals().set("state", State::Idle).unwrap();

    lua.globals()
        .set(
            "hits",
            lua.create_function(|_, ()| Ok(HITS.load(Ordering::Relaxed)))
                .unwrap(),
        )
        .unwrap();

    lua.load(include_str!("./on_set_enum.lua")).exec().unwrap();
}
