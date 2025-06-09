#![allow(incomplete_features, reason = "This is the only way to make it work")]
#![feature(never_type)]
#![feature(specialization)]

mod trait_helpers;
use {
    mlua::{FromLua, IntoLua},
    std::{
        collections::{BTreeMap, BTreeSet, HashMap, HashSet},
        hash::Hash,
    },
};
pub use {mlua_gen_macros::mlua_gen, trait_helpers::*};

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

#[macro_export]
macro_rules! to_lua {
    ($lua:ident, $($struct_or_enum:ident),*) => {
        $($struct_or_enum::to_globals(&$lua)?);*
    };
}

/// A trait that allows the usage of `__newindex` metamethod in Lua.
pub trait NewIndex {
    type Key;
    type Item;
    fn new_index(&mut self, index: Self::Key, item: Self::Item);
}

impl<T: Default> NewIndex for Vec<T> {
    type Item = T;
    type Key = usize;

    fn new_index(&mut self, index: Self::Key, item: Self::Item) {
        if index >= self.len() {
            self.resize_with(index + 1, Default::default);
        }

        self[index] = item;
    }
}

impl<K: Eq + Hash, V> NewIndex for HashMap<K, V> {
    type Item = V;
    type Key = K;

    fn new_index(&mut self, index: Self::Key, item: Self::Item) {
        self.insert(index, item);
    }
}

impl<K: Ord, V> NewIndex for BTreeMap<K, V> {
    type Item = V;
    type Key = K;

    fn new_index(&mut self, index: Self::Key, item: Self::Item) {
        self.insert(index, item);
    }
}

// TODO: check for incoherent data with index. e.g.
// Hashset [ "abc", "def", ]
//
// and in lua:
// ```lua
// set[1] = "xyz"
// ```
// this should modify the first value and not insert a new one.
impl<T: Hash + Eq> NewIndex for HashSet<T> {
    type Item = T;
    type Key = usize;

    fn new_index(&mut self, _index: Self::Key, item: Self::Item) {
        self.insert(item);
    }
}

impl<T: Ord> NewIndex for BTreeSet<T> {
    type Item = T;
    type Key = usize;

    fn new_index(&mut self, _index: Self::Key, item: Self::Item) {
        self.insert(item);
    }
}

// TODO: create a macro `lua_wrapper!()` that creates a wrapper around an external type and `#[mlua_gen]` on it
