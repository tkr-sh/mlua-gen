use mlua::IntoLua;
pub use mlua_gen_new::mlua_gen;

pub trait LuaBuilder<R: IntoLua, Lua, E, Table> {
    fn lua_builder(lua: &Lua) -> Result<R, E>;

    fn lua_fn_builder(lua: &Lua) -> Result<Option<Table>, E>;

    fn to_globals(lua: &Lua) -> Result<(), E>;
}
