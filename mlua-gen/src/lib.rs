use mlua::{FromLua, IntoLua};
pub use mlua_gen_macros::mlua_gen;

pub trait LuaBuilder<R: IntoLua + FromLua, Lua, E, Table> {
    /// Creates the constructor for a struct or enum.
    ///
    /// For structs, will allow doing:
    /// ```lua
    /// local unit = MyUnitStruct
    /// local unnamed = MyUnnamedStruct(value)
    /// local named = MyNamedStruct { key = value }
    /// ```
    ///
    /// And for enums:
    ///
    /// ```lua
    /// local unit = MyEnum.UnitVariant
    /// local unnamed = MyEnum.Unnamed(value)
    /// local named = MyEnum.Named { key = value }
    /// ```
    fn lua_builder(lua: &Lua) -> Result<R, E>;

    /// Creates the the constructor functions for a struct or enum.
    ///
    /// ### Note
    ///
    /// When used with [LuaBuilder::to_globals], it will register structs functions under `MyStruct_`
    fn lua_fn_builder(lua: &Lua) -> Result<Option<Table>, E>;

    /// Add a struct or enum to the global values of Lua.
    ///
    /// This function creates both enum&struct declaration but also function declaration and
    /// associated data like, field/variant access, methods etc.
    ///
    /// For structs (and not enums) function declaration (like `new`, `default`, etc.), note that
    /// you'll need to use `MyStruct_` when declared as `MyStruct`.
    fn to_globals(lua: &Lua) -> Result<(), E>;

    /// Same as [LuaBuilder::to_globals] but it will register the struct/enum with a custom name
    /// instead of the default Rust name.
    fn to_globals_as<S: AsRef<str>>(lua: &Lua, s: S) -> Result<(), E>;
}
