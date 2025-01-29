use mlua::{FromLua, IntoLua};
pub use mlua_gen_new::mlua_gen;

pub trait LuaBuilder<R: IntoLua + FromLua, Lua, E, Table> {
    fn lua_builder(lua: &Lua) -> Result<R, E>;

    fn lua_fn_builder(lua: &Lua) -> Result<Option<Table>, E>;

    fn to_globals(lua: &Lua) -> Result<(), E>;

    fn to_globals_as<S: AsRef<str>>(lua: &Lua, s: S) -> Result<(), E>;
}
