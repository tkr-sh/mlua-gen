//! Path-based nested proxy. Proxy tables carry a `(Resolver, Vec<PathStep>)`
//! and walk the path against the root on each Lua `__index`/`__newindex`.

use {
    mlua::{AnyUserData, FromLua, Lua, Table, Value},
    std::{
        collections::{BTreeMap, HashMap},
        hash::Hash,
        sync::Arc,
    },
};

/// One hop in a path rooted at the parent `AnyUserData`.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub enum PathStep {
    Field(&'static str),
    /// 0-based tuple index.
    Tuple(usize),
    /// Raw Lua key; the generated arm converts it.
    Index(Value),
    #[allow(dead_code, reason = "enum variants land in a later pass")]
    Variant(&'static str),
}

/// Type-erased walkers + `on_set` hook for one root. Cheap to clone.
#[doc(hidden)]
#[derive(Clone)]
pub struct Resolver {
    pub get:    Arc<dyn Fn(&Lua, &[PathStep]) -> mlua::Result<Value> + Send + Sync>,
    pub set:    Arc<dyn Fn(&Lua, &[PathStep], Value) -> mlua::Result<()> + Send + Sync>,
    pub on_set: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Resolver {
    pub fn fire_on_set(&self) {
        if let Some(hook) = &self.on_set {
            hook();
        }
    }
}

/// Build a resolver for `root: T`. Sub-proxies clone the result; `T` is
/// erased after this point.
#[doc(hidden)]
pub fn make_resolver<T: MluaGenProject + 'static>(
    root: AnyUserData,
    on_set: Option<Arc<dyn Fn() + Send + Sync>>,
) -> Resolver {
    // `AnyUserData::clone` duplicates the registry reference, not `T`: both
    // closures below borrow the same underlying userdata. Sub-proxies share
    // these closures via `Arc`, so at most two registry-handle clones exist
    // per top-level proxy entry, regardless of nesting depth.
    let root_get = root.clone();
    let root_set = root;
    Resolver {
        get: Arc::new(move |lua, steps| {
            crate::with_parent::<T, _>(&root_get, |this| this.project_get(lua, steps))
        }),
        set: Arc::new(move |lua, steps, value| {
            crate::with_parent_mut::<T, _>(&root_set, |this| this.project_set(lua, steps, value))
        }),
        on_set,
    }
}

/// Generated per `#[mlua_gen]` type. `project_*` walk one step;
/// `build_proxy` returns a Lua table whose meta-methods route through
/// `Resolver`.
#[doc(hidden)]
pub trait MluaGenProject {
    fn project_get(&self, lua: &Lua, steps: &[PathStep]) -> mlua::Result<Value>;
    fn project_set(&mut self, lua: &Lua, steps: &[PathStep], value: Value) -> mlua::Result<()>;
    fn build_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table>;
}

/// Specialization probe: forwards to `MluaGenProject` when available, else
/// errors. Lets unreachable codegen arms type-check on non-`mlua_gen` types.
#[doc(hidden)]
pub trait MluaGenProjectMaybe {
    fn maybe_project_get(&self, lua: &Lua, steps: &[PathStep]) -> mlua::Result<Value>;
    fn maybe_project_set(
        &mut self,
        lua: &Lua,
        steps: &[PathStep],
        value: Value,
    ) -> mlua::Result<()>;
    fn maybe_build_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table>;
}

impl<T> MluaGenProjectMaybe for T {
    default fn maybe_project_get(&self, _lua: &Lua, _steps: &[PathStep]) -> mlua::Result<Value> {
        Err(mlua::Error::runtime("type is not #[mlua_gen]"))
    }

    default fn maybe_project_set(
        &mut self,
        _lua: &Lua,
        _steps: &[PathStep],
        _value: Value,
    ) -> mlua::Result<()> {
        Err(mlua::Error::runtime("type is not #[mlua_gen]"))
    }

    default fn maybe_build_proxy(
        _lua: &Lua,
        _ctx: Resolver,
        _path: Vec<PathStep>,
        _vis: Visibility,
    ) -> mlua::Result<Table> {
        Err(mlua::Error::runtime("type is not #[mlua_gen]"))
    }
}

impl<T: MluaGenProject> MluaGenProjectMaybe for T {
    fn maybe_project_get(&self, lua: &Lua, steps: &[PathStep]) -> mlua::Result<Value> {
        <T as MluaGenProject>::project_get(self, lua, steps)
    }

    fn maybe_project_set(
        &mut self,
        lua: &Lua,
        steps: &[PathStep],
        value: Value,
    ) -> mlua::Result<()> {
        <T as MluaGenProject>::project_set(self, lua, steps, value)
    }

    fn maybe_build_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table> {
        <T as MluaGenProject>::build_proxy(lua, ctx, path, vis)
    }
}

#[doc(hidden)]
pub fn bad_step(context: &str) -> mlua::Error {
    mlua::Error::runtime(format!("invalid path step: {context}"))
}

/// Top-level proxy gating. Sub-proxies always use `Both`; only the entry
/// table respects the parent field's `set` visibility.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Both,
    /// `__newindex` omitted; Lua falls back to `rawset` on the proxy table.
    GetOnly,
}

/// Proxy table for a collection of leaf values: `__index(i)` reads,
/// `__newindex(i, v)` writes via the resolver and fires `on_set`.
#[doc(hidden)]
pub fn build_indexed_proxy_leaf(
    lua: &Lua,
    ctx: Resolver,
    path: Vec<PathStep>,
    vis: Visibility,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let mt = lua.create_table()?;

    let ctx_g = ctx.clone();
    let path_g = path.clone();
    mt.set(
        "__index",
        lua.create_function(move |lua, (_, key): (Value, Value)| {
            let mut p = path_g.clone();
            p.push(PathStep::Index(key));
            (ctx_g.get)(lua, &p)
        })?,
    )?;

    if vis == Visibility::Both {
        let ctx_s = ctx;
        let path_s = path;
        mt.set(
            "__newindex",
            lua.create_function(move |lua, (_, key, value): (Value, Value, Value)| {
                let mut p = path_s.clone();
                p.push(PathStep::Index(key));
                (ctx_s.set)(lua, &p, value)?;
                ctx_s.fire_on_set();
                Ok(())
            })?,
        )?;
    }

    table.set_metatable(Some(mt));
    Ok(table)
}

/// Proxy table for a collection of `mlua_gen` elements: `__index(i)`
/// returns a sub-proxy; `__newindex(i, v)` replaces the whole element.
#[doc(hidden)]
pub fn build_indexed_proxy_struct<Elem: MluaGenProject + 'static>(
    lua: &Lua,
    ctx: Resolver,
    path: Vec<PathStep>,
    vis: Visibility,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let mt = lua.create_table()?;

    let ctx_g = ctx.clone();
    let path_g = path.clone();
    mt.set(
        "__index",
        lua.create_function(move |lua, (_, key): (Value, Value)| {
            let mut p = path_g.clone();
            p.push(PathStep::Index(key));
            Ok(Value::Table(<Elem as MluaGenProject>::build_proxy(
                lua,
                ctx_g.clone(),
                p,
                Visibility::Both,
            )?))
        })?,
    )?;

    if vis == Visibility::Both {
        let ctx_s = ctx;
        let path_s = path;
        mt.set(
            "__newindex",
            lua.create_function(move |lua, (_, key, value): (Value, Value, Value)| {
                let mut p = path_s.clone();
                p.push(PathStep::Index(key));
                (ctx_s.set)(lua, &p, value)?;
                ctx_s.fire_on_set();
                Ok(())
            })?,
        )?;
    }

    table.set_metatable(Some(mt));
    Ok(table)
}

/// Indexable collection of `MluaGenProject` elements. Specialized for
/// `Vec`, `HashMap`, `BTreeMap`; default impl errors.
#[doc(hidden)]
pub trait CollectionProject {
    const IS_COLLECTION_OF_MLUA_GEN: bool = false;

    fn project_get_elem(&self, lua: &Lua, key: Value, steps: &[PathStep]) -> mlua::Result<Value>;

    fn project_set_elem(
        &mut self,
        lua: &Lua,
        key: Value,
        steps: &[PathStep],
        value: Value,
    ) -> mlua::Result<()>;

    fn build_collection_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table>;
}

impl<C> CollectionProject for C {
    default const IS_COLLECTION_OF_MLUA_GEN: bool = false;

    default fn project_get_elem(
        &self,
        _lua: &Lua,
        _key: Value,
        _steps: &[PathStep],
    ) -> mlua::Result<Value> {
        Err(mlua::Error::runtime("not a collection of mlua_gen"))
    }

    default fn project_set_elem(
        &mut self,
        _lua: &Lua,
        _key: Value,
        _steps: &[PathStep],
        _value: Value,
    ) -> mlua::Result<()> {
        Err(mlua::Error::runtime("not a collection of mlua_gen"))
    }

    default fn build_collection_proxy(
        _lua: &Lua,
        _ctx: Resolver,
        _path: Vec<PathStep>,
        _vis: Visibility,
    ) -> mlua::Result<Table> {
        Err(mlua::Error::runtime("not a collection of mlua_gen"))
    }
}

impl<T: MluaGenProject + 'static> CollectionProject for Vec<T> {
    const IS_COLLECTION_OF_MLUA_GEN: bool = true;

    fn project_get_elem(&self, lua: &Lua, key: Value, steps: &[PathStep]) -> mlua::Result<Value> {
        let one_based: usize = FromLua::from_lua(key, lua)?;
        let idx = one_based
            .checked_sub(1)
            .ok_or_else(|| mlua::Error::runtime("Lua indices start at 1"))?;
        let elem = self
            .get(idx)
            .ok_or_else(|| mlua::Error::runtime("index out of bounds"))?;
        elem.project_get(lua, steps)
    }

    fn project_set_elem(
        &mut self,
        lua: &Lua,
        key: Value,
        steps: &[PathStep],
        value: Value,
    ) -> mlua::Result<()> {
        let one_based: usize = FromLua::from_lua(key, lua)?;
        let idx = one_based
            .checked_sub(1)
            .ok_or_else(|| mlua::Error::runtime("Lua indices start at 1"))?;
        let elem = self
            .get_mut(idx)
            .ok_or_else(|| mlua::Error::runtime("index out of bounds"))?;
        elem.project_set(lua, steps, value)
    }

    fn build_collection_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table> {
        build_indexed_proxy_struct::<T>(lua, ctx, path, vis)
    }
}

impl<K, V> CollectionProject for HashMap<K, V>
where
    K: Eq + Hash + FromLua + 'static,
    V: MluaGenProject + 'static,
{
    const IS_COLLECTION_OF_MLUA_GEN: bool = true;

    fn project_get_elem(&self, lua: &Lua, key: Value, steps: &[PathStep]) -> mlua::Result<Value> {
        let key = K::from_lua(key, lua)?;
        let elem = self
            .get(&key)
            .ok_or_else(|| mlua::Error::runtime("key not found"))?;
        elem.project_get(lua, steps)
    }

    fn project_set_elem(
        &mut self,
        lua: &Lua,
        key: Value,
        steps: &[PathStep],
        value: Value,
    ) -> mlua::Result<()> {
        let key = K::from_lua(key, lua)?;
        let elem = self
            .get_mut(&key)
            .ok_or_else(|| mlua::Error::runtime("key not found"))?;
        elem.project_set(lua, steps, value)
    }

    fn build_collection_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table> {
        build_indexed_proxy_struct::<V>(lua, ctx, path, vis)
    }
}

impl<K, V> CollectionProject for BTreeMap<K, V>
where
    K: Ord + FromLua + 'static,
    V: MluaGenProject + 'static,
{
    const IS_COLLECTION_OF_MLUA_GEN: bool = true;

    fn project_get_elem(&self, lua: &Lua, key: Value, steps: &[PathStep]) -> mlua::Result<Value> {
        let key = K::from_lua(key, lua)?;
        let elem = self
            .get(&key)
            .ok_or_else(|| mlua::Error::runtime("key not found"))?;
        elem.project_get(lua, steps)
    }

    fn project_set_elem(
        &mut self,
        lua: &Lua,
        key: Value,
        steps: &[PathStep],
        value: Value,
    ) -> mlua::Result<()> {
        let key = K::from_lua(key, lua)?;
        let elem = self
            .get_mut(&key)
            .ok_or_else(|| mlua::Error::runtime("key not found"))?;
        elem.project_set(lua, steps, value)
    }

    fn build_collection_proxy(
        lua: &Lua,
        ctx: Resolver,
        path: Vec<PathStep>,
        vis: Visibility,
    ) -> mlua::Result<Table> {
        build_indexed_proxy_struct::<V>(lua, ctx, path, vis)
    }
}
