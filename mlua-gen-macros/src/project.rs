//! Codegen for `impl MluaGenProject`. One arm per field/tuple-index;
//! either recurses into the child's `project_*` or handles a leaf inline.

use {
    crate::{attr::MinimalField, shared::remove_ty_from_generics},
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    syn::{Fields, Generics, Ident},
};

pub(crate) fn impl_project(
    name: &Ident,
    generics: &Generics,
    all_fields: &Fields,
    get_fields: &[MinimalField],
    set_fields: &[MinimalField],
) -> TokenStream2 {
    let non_typed_generics = remove_ty_from_generics(generics);

    let (get_arms, set_arms) = match all_fields {
        Fields::Named(_) => named_arms(get_fields, set_fields),
        Fields::Unnamed(_) => unnamed_arms(get_fields, set_fields),
        Fields::Unit => (quote!(), quote!()),
    };

    let build_proxy_body = build_proxy_body(all_fields, get_fields, set_fields);

    quote! {
        impl #generics ::mlua_gen::MluaGenProject for #name #non_typed_generics {
            fn project_get(
                &self,
                lua: &::mlua::Lua,
                steps: &[::mlua_gen::PathStep],
            ) -> ::mlua::Result<::mlua::Value> {
                let Some((step, rest)) = steps.split_first() else {
                    return Err(::mlua::Error::runtime(
                        "empty path: project_get on root must go through resolve_get",
                    ));
                };
                match step {
                    #get_arms
                    _ => Err(::mlua_gen::bad_step(stringify!(#name))),
                }
            }

            fn project_set(
                &mut self,
                lua: &::mlua::Lua,
                steps: &[::mlua_gen::PathStep],
                __mlua_gen_value: ::mlua::Value,
            ) -> ::mlua::Result<()> {
                let Some((step, rest)) = steps.split_first() else {
                    *self = ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                    return Ok(());
                };
                match step {
                    #set_arms
                    _ => Err(::mlua_gen::bad_step(stringify!(#name))),
                }
            }

            fn build_proxy(
                lua: &::mlua::Lua,
                ctx: ::mlua_gen::Resolver,
                path: ::std::vec::Vec<::mlua_gen::PathStep>,
                vis: ::mlua_gen::Visibility,
            ) -> ::mlua::Result<::mlua::Table> {
                #build_proxy_body
            }
        }
    }
}

fn named_arms(
    get_fields: &[MinimalField],
    set_fields: &[MinimalField],
) -> (TokenStream2, TokenStream2) {
    let get = get_fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        let name = &f.ident_string;
        let body = field_get_body(&quote!(self.#ident), ty, name);
        quote! {
            ::mlua_gen::PathStep::Field(#name) => { #body }
        }
    });

    let set = set_fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        let name = &f.ident_string;
        let body = field_set_body(&quote!(self.#ident), ty, name);
        quote! {
            ::mlua_gen::PathStep::Field(#name) => { #body }
        }
    });

    (quote!(#(#get)*), quote!(#(#set)*))
}

fn build_proxy_body(
    all_fields: &Fields,
    get_fields: &[MinimalField],
    set_fields: &[MinimalField],
) -> TokenStream2 {
    match all_fields {
        Fields::Named(_) => build_proxy_named(get_fields, set_fields),
        Fields::Unnamed(_) => build_proxy_unnamed(get_fields, set_fields),
        Fields::Unit => {
            quote! {
                Err(::mlua::Error::runtime("unit struct has no fields to proxy"))
            }
        },
    }
}

/// Helper-fn definitions shared by every proxy `__index` / `__newindex`
/// closure: monomorphised per field type so the const probes prune to a
/// single arm at codegen.
pub(crate) fn proxy_dispatch_helpers() -> TokenStream2 {
    quote! {
        fn proxy_index_dispatch<Ty>(
            lua: &::mlua::Lua,
            ctx: ::mlua_gen::Resolver,
            p: ::std::vec::Vec<::mlua_gen::PathStep>,
        ) -> ::mlua::Result<::mlua::Value>
        where
            Ty: ::mlua_gen::MluaGenProjectMaybe
                + ::mlua_gen::IsMluaGenerated
                + ::mlua_gen::CollectionProject
                + ::mlua_gen::IsIndexable,
        {
            match (
                <Ty as ::mlua_gen::IsMluaGenerated>::IS_MLUA_GENERATED,
                <Ty as ::mlua_gen::CollectionProject>::IS_COLLECTION_OF_MLUA_GEN,
                <Ty as ::mlua_gen::IsIndexable>::IS_INDEXABLE,
            ) {
                (true, _, _) => Ok(::mlua::Value::Table(
                    <Ty as ::mlua_gen::MluaGenProjectMaybe>::maybe_build_proxy(
                        lua, ctx, p, ::mlua_gen::Visibility::Both,
                    )?,
                )),
                (_, true, _) => Ok(::mlua::Value::Table(
                    <Ty as ::mlua_gen::CollectionProject>::build_collection_proxy(
                        lua, ctx, p, ::mlua_gen::Visibility::Both,
                    )?,
                )),
                (_, _, true) => Ok(::mlua::Value::Table(
                    ::mlua_gen::build_indexed_proxy_leaf(
                        lua, ctx, p, ::mlua_gen::Visibility::Both,
                    )?,
                )),
                (false, false, false) => (ctx.get)(lua, &p),
            }
        }

        fn proxy_newindex_dispatch<Ty>(
            lua: &::mlua::Lua,
            ctx: ::mlua_gen::Resolver,
            p: ::std::vec::Vec<::mlua_gen::PathStep>,
            value: ::mlua::Value,
        ) -> ::mlua::Result<()>
        where
            Ty: ::mlua_gen::MluaGenProjectMaybe
                + ::mlua_gen::IsMluaGenerated
                + ::mlua_gen::CollectionProject
                + ::mlua_gen::IsIndexable,
        {
            (ctx.set)(lua, &p, value)?;
            ctx.fire_on_set();
            Ok(())
        }
    }
}

fn build_proxy_named(get_fields: &[MinimalField], set_fields: &[MinimalField]) -> TokenStream2 {
    let index_arms = get_fields.iter().map(|f| {
        let ty = &f.ty;
        let name = &f.ident_string;
        quote! {
            #name => {
                let mut p = path_g.clone();
                p.push(::mlua_gen::PathStep::Field(#name));
                proxy_index_dispatch::<#ty>(lua, ctx_g.clone(), p)
            }
        }
    });

    let newindex_arms = set_fields.iter().map(|f| {
        let ty = &f.ty;
        let name = &f.ident_string;
        quote! {
            #name => {
                let mut p = path_s.clone();
                p.push(::mlua_gen::PathStep::Field(#name));
                proxy_newindex_dispatch::<#ty>(lua, ctx_s.clone(), p, value)
            }
        }
    });

    let helpers = proxy_dispatch_helpers();
    quote! {
        #helpers

        let table = lua.create_table()?;
        let mt = lua.create_table()?;

        let ctx_g = ctx.clone();
        let path_g = path.clone();
        mt.set(
            "__index",
            lua.create_function(move |lua, (_, key): (::mlua::Value, ::mlua::Value)| -> ::mlua::Result<::mlua::Value> {
                let key_str: ::std::string::String =
                    ::mlua::FromLua::from_lua(key, lua)?;
                match key_str.as_str() {
                    #(#index_arms)*
                    _ => Err(::mlua::Error::runtime(::std::format!(
                        "no such field: {key_str}"
                    ))),
                }
            })?,
        )?;

        if vis == ::mlua_gen::Visibility::Both {
            let ctx_s = ctx;
            let path_s = path;
            mt.set(
                "__newindex",
                lua.create_function(move |lua, (_, key, value): (::mlua::Value, ::mlua::Value, ::mlua::Value)| -> ::mlua::Result<()> {
                    let key_str: ::std::string::String =
                        ::mlua::FromLua::from_lua(key, lua)?;
                    match key_str.as_str() {
                        #(#newindex_arms)*
                        _ => Err(::mlua::Error::runtime(::std::format!(
                            "no such field: {key_str}"
                        ))),
                    }
                })?,
            )?;
        }

        table.set_metatable(Some(mt));
        Ok(table)
    }
}

fn build_proxy_unnamed(get_fields: &[MinimalField], set_fields: &[MinimalField]) -> TokenStream2 {
    // Lua keys are 1-based; PathStep::Tuple is 0-based.
    let index_arms = get_fields.iter().map(|f| {
        let ty = &f.ty;
        let zero_based: usize = f.ident_string.parse().expect("tuple field must be numeric");
        let lua_index: usize = zero_based + 1;
        quote! {
            #lua_index => {
                let mut p = path_g.clone();
                p.push(::mlua_gen::PathStep::Tuple(#zero_based));
                proxy_index_dispatch::<#ty>(lua, ctx_g.clone(), p)
            }
        }
    });

    let newindex_arms = set_fields.iter().map(|f| {
        let ty = &f.ty;
        let zero_based: usize = f.ident_string.parse().expect("tuple field must be numeric");
        let lua_index: usize = zero_based + 1;
        quote! {
            #lua_index => {
                let mut p = path_s.clone();
                p.push(::mlua_gen::PathStep::Tuple(#zero_based));
                proxy_newindex_dispatch::<#ty>(lua, ctx_s.clone(), p, value)
            }
        }
    });

    let helpers = proxy_dispatch_helpers();
    quote! {
        #helpers

        let table = lua.create_table()?;
        let mt = lua.create_table()?;

        let ctx_g = ctx.clone();
        let path_g = path.clone();
        mt.set(
            "__index",
            lua.create_function(move |lua, (_, key): (::mlua::Value, usize)| -> ::mlua::Result<::mlua::Value> {
                match key {
                    #(#index_arms)*
                    _ => Err(::mlua::Error::runtime(::std::format!(
                        "no such tuple field: {key}"
                    ))),
                }
            })?,
        )?;

        if vis == ::mlua_gen::Visibility::Both {
            let ctx_s = ctx;
            let path_s = path;
            mt.set(
                "__newindex",
                lua.create_function(move |lua, (_, key, value): (::mlua::Value, usize, ::mlua::Value)| -> ::mlua::Result<()> {
                    match key {
                        #(#newindex_arms)*
                        _ => Err(::mlua::Error::runtime(::std::format!(
                            "no such tuple field: {key}"
                        ))),
                    }
                })?,
            )?;
        }

        table.set_metatable(Some(mt));
        Ok(table)
    }
}

fn unnamed_arms(
    get_fields: &[MinimalField],
    set_fields: &[MinimalField],
) -> (TokenStream2, TokenStream2) {
    let get = get_fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        let parsed: usize = f.ident_string.parse().expect("tuple field must be numeric");
        let body = field_get_body(&quote!(self.#ident), ty, &f.ident_string);
        quote! {
            ::mlua_gen::PathStep::Tuple(#parsed) => { #body }
        }
    });

    let set = set_fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        let parsed: usize = f.ident_string.parse().expect("tuple field must be numeric");
        let body = field_set_body(&quote!(self.#ident), ty, &f.ident_string);
        quote! {
            ::mlua_gen::PathStep::Tuple(#parsed) => { #body }
        }
    });

    (quote!(#(#get)*), quote!(#(#set)*))
}

pub(crate) fn field_get_body(access: &TokenStream2, ty: &syn::Type, name: &str) -> TokenStream2 {
    quote! {
        match (
            <#ty as ::mlua_gen::IsMluaGenerated>::IS_MLUA_GENERATED,
            <#ty as ::mlua_gen::CollectionProject>::IS_COLLECTION_OF_MLUA_GEN,
            <#ty as ::mlua_gen::IsIndexable>::IS_INDEXABLE,
        ) {
            (true, _, _) => <#ty as ::mlua_gen::MluaGenProjectMaybe>::maybe_project_get(&#access, lua, rest),
            (_, true, _) => match rest.split_first() {
                None => ::mlua::IntoLua::into_lua(#access.clone(), lua),
                Some((::mlua_gen::PathStep::Index(k), rest2)) => {
                    <#ty as ::mlua_gen::CollectionProject>::project_get_elem(
                        &#access, lua, k.clone(), rest2,
                    )
                },
                Some(_) => Err(::mlua_gen::bad_step(#name)),
            },
            (_, _, true) => match rest.split_first() {
                None => ::mlua::IntoLua::into_lua(#access.clone(), lua),
                Some((::mlua_gen::PathStep::Index(k), rest2)) if rest2.is_empty() => {
                    use ::mlua_gen::IsIndexable;
                    let one_based: usize = ::mlua::FromLua::from_lua(k.clone(), lua)?;
                    let idx = one_based
                        .checked_sub(1)
                        .ok_or_else(|| ::mlua::Error::runtime("Lua indices start at 1"))?;
                    ::mlua::IntoLua::into_lua(#access.index_or_unreachable(idx), lua)
                },
                Some(_) => Err(::mlua_gen::bad_step(#name)),
            },
            (false, false, false) => {
                if rest.is_empty() {
                    ::mlua::IntoLua::into_lua(#access.clone(), lua)
                } else {
                    Err(::mlua_gen::bad_step(#name))
                }
            },
        }
    }
}

pub(crate) fn field_set_body(access: &TokenStream2, ty: &syn::Type, name: &str) -> TokenStream2 {
    quote! {
        match (
            <#ty as ::mlua_gen::IsMluaGenerated>::IS_MLUA_GENERATED,
            <#ty as ::mlua_gen::CollectionProject>::IS_COLLECTION_OF_MLUA_GEN,
            <#ty as ::mlua_gen::IsIndexable>::IS_INDEXABLE,
        ) {
            (true, _, _) => {
                if rest.is_empty() {
                    #access = ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                    Ok(())
                } else {
                    <#ty as ::mlua_gen::MluaGenProjectMaybe>::maybe_project_set(
                        &mut #access, lua, rest, __mlua_gen_value,
                    )
                }
            },
            (_, true, _) => match rest.split_first() {
                None => {
                    #access = ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                    Ok(())
                },
                Some((::mlua_gen::PathStep::Index(k), rest2)) => {
                    <#ty as ::mlua_gen::CollectionProject>::project_set_elem(
                        &mut #access, lua, k.clone(), rest2, __mlua_gen_value,
                    )
                },
                Some(_) => Err(::mlua_gen::bad_step(#name)),
            },
            (_, _, true) => match rest.split_first() {
                None => {
                    #access = ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                    Ok(())
                },
                Some((::mlua_gen::PathStep::Index(k), rest2)) if rest2.is_empty() => {
                    if <#ty as ::mlua_gen::IsNewIndexable>::IS_NEW_INDEXABLE {
                        use ::mlua_gen::IsNewIndexable;
                        let key: <#ty as IsNewIndexable>::Key =
                            ::mlua::FromLua::from_lua(k.clone(), lua)?;
                        let item: <#ty as IsNewIndexable>::Item =
                            ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                        #access.set_index_or_unreachable(key, item);
                        Ok(())
                    } else if <#ty as ::mlua_gen::IsMutIndexable>::IS_MUT_INDEXABLE {
                        use ::mlua_gen::IsMutIndexable;
                        let one_based: usize = ::mlua::FromLua::from_lua(k.clone(), lua)?;
                        let idx = one_based
                            .checked_sub(1)
                            .ok_or_else(|| ::mlua::Error::runtime("Lua indices start at 1"))?;
                        let item: <#ty as IsMutIndexable>::IndexType =
                            ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                        #access.set_index_or_unreachable(idx, item);
                        Ok(())
                    } else {
                        Err(::mlua_gen::bad_step(#name))
                    }
                },
                Some(_) => Err(::mlua_gen::bad_step(#name)),
            },
            (false, false, false) => {
                if rest.is_empty() {
                    #access = ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                    Ok(())
                } else {
                    Err(::mlua_gen::bad_step(#name))
                }
            },
        }
    }
}
