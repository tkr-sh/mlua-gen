use {
    crate::{
        attr::MethodOrFunction,
        builder::{builder_for_fields, builder_for_functions},
        project::{field_get_body, field_set_body, proxy_dispatch_helpers},
        shared::remove_ty_from_generics,
    },
    proc_macro2::{Span, TokenStream as TokenStream2},
    quote::quote,
    std::iter::repeat_with,
    syn::{DataEnum, Generics, Ident, Path, Variant},
};

/// Function that impl the `mlua_gen::LuaBuilder` trait for an enum
pub fn builder(
    name: &Ident,
    de: &DataEnum,
    functions: Vec<&MethodOrFunction>,
    generics: &Generics,
) -> TokenStream2 {
    let (names, builders): (Vec<_>, Vec<_>) = de
        .variants
        .iter()
        .map(|v| {
            let var_name = &v.ident;
            (
                var_name,
                builder_for_fields(&quote! {Self::#var_name}, &v.fields, false),
            )
        })
        .unzip();
    let no_ty_generics = remove_ty_from_generics(generics);

    let builder_fn_code = builder_for_functions(&quote! {Self}, functions);

    quote! {
        impl #generics ::mlua_gen::LuaBuilder<
            ::mlua::Table,
            ::mlua::Lua,
            ::mlua::Error,
            ::mlua::Table,
        > for #name #no_ty_generics {
            fn lua_builder(lua: &::mlua::Lua) -> ::mlua::Result<::mlua::Table> {
                let enum_variants_table = lua.create_table()?;
                #( enum_variants_table.set(stringify!(#names), #builders?)?; )*
                Ok(enum_variants_table)
            }

            fn lua_fn_builder(lua: &::mlua::Lua) -> ::mlua::Result<Option<::mlua::Table>> {
                #builder_fn_code
            }

            fn to_globals(lua: &::mlua::Lua) -> ::mlua::Result<()> {
                Self::to_globals_as(lua, stringify!(#name))
            }

            fn to_globals_as<S: AsRef<str>>(lua: &::mlua::Lua, s: S) -> ::mlua::Result<()> {
                let table = Self::lua_builder(&lua)?;

                if let Some(table_to_extend_with) = Self::lua_fn_builder(&lua)? {
                    // Equivalent to extend, tho, doesn't seems to exists
                    for pairs in table_to_extend_with.pairs() {
                        let (k,v): (String, ::mlua::Function) = pairs?;

                        table.set(k, v)?;
                    }
                }

                lua.globals()
                    .set(s.as_ref(), Self::lua_builder(&lua)?)?;

                Ok(())
            }
        }

        impl #generics ::mlua_gen::AutomaticImplWhenMluaGen for #name #no_ty_generics { }
    }
}

enum VariantKind {
    Named,
    Unnamed,
    Unit,
}

/// Per-variant codegen pieces used to build the `UserData` and
/// `MluaGenProject` impls in one pass.
struct VariantPieces {
    kind:            VariantKind,
    accessor:        String,
    fields_arm:      TokenStream2,
    project_get_arm: TokenStream2,
    project_set_arm: TokenStream2,
    build_proxy_arm: TokenStream2,
    from_lua_match:  TokenStream2,
}

pub(crate) fn user_data<'l, I: Iterator<Item = &'l Variant>>(
    name: &Ident,
    generics: &Generics,
    variants: I,
    custom_field: Option<syn::Ident>,
    custom_method_or_fn: Option<syn::Ident>,
    on_set: Option<Path>,
) -> proc_macro2::TokenStream {
    let on_set_call = match &on_set {
        Some(path) => quote!( (#path)(); ),
        None => quote!(),
    };
    let on_set_resolver = match &on_set {
        Some(path) => {
            quote! {
                ::std::option::Option::Some(::std::sync::Arc::new(|| { (#path)(); })
                    as ::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>)
            }
        },
        None => quote!(::std::option::Option::None),
    };

    let pieces: Vec<VariantPieces> = variants
        .map(|variant| build_variant_pieces(variant, &on_set_call, &on_set_resolver))
        .collect();

    let accessors: Vec<&String> = pieces.iter().map(|p| &p.accessor).collect();
    let fields_arms = pieces.iter().map(|p| &p.fields_arm);
    let project_get_arms = pieces.iter().map(|p| &p.project_get_arm);
    let project_set_arms = pieces.iter().map(|p| &p.project_set_arm);
    let build_proxy_arms = pieces.iter().map(|p| &p.build_proxy_arm);
    let from_lua_match = pieces.iter().map(|p| &p.from_lua_match);

    let router_get_arms = pieces.iter().map(|p| {
        let s = &p.accessor;
        let is_unit = matches!(p.kind, VariantKind::Unit);
        if is_unit {
            quote! {
                #s => {
                    let mut p = path_g.clone();
                    p.push(::mlua_gen::PathStep::Variant(#s));
                    (ctx_g.get)(lua, &p)
                }
            }
        } else {
            quote! {
                #s => {
                    // Inactive variant resolves to nil (matches top-level accessor).
                    let mut p = path_g.clone();
                    p.push(::mlua_gen::PathStep::Variant(#s));
                    let active_probe = (ctx_g.get)(lua, &p)?;
                    if matches!(active_probe, ::mlua::Value::Nil) {
                        return Ok(::mlua::Value::Nil);
                    }
                    let table = <Self as ::mlua_gen::MluaGenProject>::build_proxy(
                        lua, ctx_g.clone(), p, ::mlua_gen::Visibility::Both,
                    )?;
                    Ok(::mlua::Value::Table(table))
                }
            }
        }
    });
    let router_set_arms = accessors.iter().map(|a| {
        let s: &str = a;
        quote! {
            #s => {
                let mut p = path_s.clone();
                p.push(::mlua_gen::PathStep::Variant(#s));
                (ctx_s.set)(lua, &p, value)?;
                ctx_s.fire_on_set();
                Ok(())
            }
        }
    });

    let extra_fields = if let Some(field) = custom_field {
        quote! {#field(reserved_fields)}
    } else {
        quote!()
    };

    let extra_impls = if let Some(method_or_fn) = custom_method_or_fn {
        quote!(#method_or_fn(methods))
    } else {
        quote!()
    };

    let non_typed_generics = remove_ty_from_generics(generics);

    let proxy_helpers = proxy_dispatch_helpers();

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
                    ::mlua_gen::PathStep::Variant(__variant) => match *__variant {
                        #(#project_get_arms)*
                        _ => Err(::mlua_gen::bad_step(stringify!(#name))),
                    },
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
                    ::mlua_gen::PathStep::Variant(__variant) => match *__variant {
                        #(#project_set_arms)*
                        _ => Err(::mlua_gen::bad_step(stringify!(#name))),
                    },
                    _ => Err(::mlua_gen::bad_step(stringify!(#name))),
                }
            }

            fn build_proxy(
                lua: &::mlua::Lua,
                ctx: ::mlua_gen::Resolver,
                path: ::std::vec::Vec<::mlua_gen::PathStep>,
                vis: ::mlua_gen::Visibility,
            ) -> ::mlua::Result<::mlua::Table> {
                #proxy_helpers

                // Path ending in `Variant(name)` → per-variant proxy.
                // Otherwise → router proxy whose `__index(name)` extends the path.
                if let Some(::mlua_gen::PathStep::Variant(__variant)) = path.last().cloned() {
                    match __variant {
                        #(#build_proxy_arms)*
                        _ => Err(::mlua::Error::runtime(::std::format!(
                            "no such variant: {}", __variant
                        ))),
                    }
                } else {
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
                                #(#router_get_arms)*
                                _ => Err(::mlua::Error::runtime(::std::format!(
                                    "no such variant: {key_str}"
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
                                    #(#router_set_arms)*
                                    _ => Err(::mlua::Error::runtime(::std::format!(
                                        "no such variant: {key_str}"
                                    ))),
                                }
                            })?,
                        )?;
                    }

                    table.set_metatable(Some(mt));
                    Ok(table)
                }
            }
        }

        impl #generics #name #non_typed_generics {
            #[doc(hidden)]
            #[allow(non_snake_case, dead_code)]
            fn __mlua_gen_enum_marker() {}
        }

        impl #generics ::mlua::FromLua for #name #non_typed_generics {
            fn from_lua(value: ::mlua::Value, lua: &::mlua::Lua) -> ::mlua::Result<#name #non_typed_generics> {
                match value {
                    ::mlua::Value::Table(table) => {
                        #(#from_lua_match)*
                        Err(::mlua::Error::runtime("No valid variant found."))
                    },
                    ::mlua::Value::UserData(user_data) => {
                        user_data.take()
                    },
                    val => Err(::mlua::Error::runtime(format!("Expected a table or a UserData. Got: {val:?}"))),
                }
            }
        }

        impl #generics ::mlua::UserData for #name #non_typed_generics {
            fn add_fields<MluaUserDataFields: ::mlua::UserDataFields<Self>>(reserved_fields: &mut MluaUserDataFields) {
                #(#fields_arms)*
                ;
                #extra_fields
            }

            fn add_methods<MluaUserDataMethods: ::mlua::UserDataMethods<Self>>(methods: &mut MluaUserDataMethods) {
                #extra_impls
            }
        }
    }
}

fn build_variant_pieces(
    variant: &Variant,
    on_set_call: &TokenStream2,
    on_set_resolver: &TokenStream2,
) -> VariantPieces {
    let original_variant_string = variant.ident.to_string();
    let variant_ident = syn::Ident::new(&original_variant_string, Span::call_site());
    let accessor = original_variant_string.to_lowercase();

    match &variant.fields {
        syn::Fields::Named(field_named) => {
            let field_idents: Vec<&syn::Ident> = field_named
                .named
                .iter()
                .map(|f| f.ident.as_ref().expect("Is named"))
                .collect();
            let field_tys: Vec<&syn::Type> = field_named.named.iter().map(|f| &f.ty).collect();
            let field_strings: Vec<String> = field_idents.iter().map(|i| i.to_string()).collect();

            let field_constructors = field_idents
                .iter()
                .zip(field_strings.iter())
                .map(|(id, s)| quote!(#id: table.get(#s)?,));
            let from_lua_match = quote! {
                if let Ok(table_value) = table.get::<::mlua::Value>(#accessor) {
                    if let ::mlua::Value::Table(table) = table_value {
                        return Ok(Self::#variant_ident { #(#field_constructors)* });
                    }
                }
            };

            let setter_constructors = field_idents
                .iter()
                .zip(field_strings.iter())
                .map(|(id, s)| quote!(#id: table.get(#s)?,));
            let fields_arm = quote! {
                reserved_fields.add_field_function_get(
                    #accessor,
                    |lua: &::mlua::Lua, this: ::mlua::AnyUserData| -> ::mlua::Result<::mlua::Value> {
                        let active = ::mlua_gen::with_parent::<Self, _>(&this, |this| {
                            Ok(matches!(this, Self::#variant_ident { .. }))
                        })?;
                        if !active {
                            return Ok(::mlua::Value::Nil);
                        }
                        let on_set: ::std::option::Option<::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>> = #on_set_resolver;
                        let ctx = ::mlua_gen::make_resolver::<Self>(this, on_set);
                        let path = ::std::vec![::mlua_gen::PathStep::Variant(#accessor)];
                        let table = <Self as ::mlua_gen::MluaGenProject>::build_proxy(
                            lua, ctx, path, ::mlua_gen::Visibility::Both,
                        )?;
                        Ok(::mlua::Value::Table(table))
                    },
                );
                reserved_fields.add_field_method_set(
                    #accessor,
                    |_, this, table: ::mlua::Table| {
                        *this = Self::#variant_ident { #(#setter_constructors)* };
                        #on_set_call
                        Ok(())
                    },
                );
            };

            let project_get_field_arms = field_idents
                .iter()
                .zip(field_tys.iter())
                .zip(field_strings.iter())
                .map(|((id, ty), s)| {
                    let body = field_get_body(&quote!((*#id)), ty, s);
                    quote! { ::mlua_gen::PathStep::Field(#s) => { #body } }
                });
            let project_set_field_arms = field_idents
                .iter()
                .zip(field_tys.iter())
                .zip(field_strings.iter())
                .map(|((id, ty), s)| {
                    let body = field_set_body(&quote!((*#id)), ty, s);
                    quote! { ::mlua_gen::PathStep::Field(#s) => { #body } }
                });

            let project_get_arm = quote! {
                #accessor => {
                    let Self::#variant_ident { #(#field_idents),* } = self else {
                        // Bare `enum.variant` on inactive → nil; deeper path → error.
                        if rest.is_empty() {
                            return Ok(::mlua::Value::Nil);
                        }
                        return Err(::mlua::Error::runtime(
                            "variant changed under proxy",
                        ));
                    };
                    let Some((step, rest)) = rest.split_first() else {
                        let table = lua.create_table()?;
                        #( table.set(#field_strings, #field_idents.to_owned())?; )*
                        return ::mlua::IntoLua::into_lua(table, lua);
                    };
                    match step {
                        #(#project_get_field_arms)*
                        _ => Err(::mlua_gen::bad_step(#accessor)),
                    }
                },
            };

            let setter_constructors2 = field_idents
                .iter()
                .zip(field_strings.iter())
                .map(|(id, s)| quote!(#id: table.get(#s)?,));
            let project_set_arm = quote! {
                #accessor => {
                    if rest.is_empty() {
                        let table: ::mlua::Table =
                            ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                        *self = Self::#variant_ident { #(#setter_constructors2)* };
                        return Ok(());
                    }
                    let Self::#variant_ident { #(#field_idents),* } = self else {
                        return Err(::mlua::Error::runtime(
                            "variant changed under proxy",
                        ));
                    };
                    let (step, rest) = rest.split_first().expect("checked above");
                    match step {
                        #(#project_set_field_arms)*
                        _ => Err(::mlua_gen::bad_step(#accessor)),
                    }
                },
            };

            let proxy_index_arms = field_idents
                .iter()
                .zip(field_tys.iter())
                .zip(field_strings.iter())
                .map(|((_id, ty), s)| {
                    quote! {
                        #s => {
                            let mut p = path_g.clone();
                            p.push(::mlua_gen::PathStep::Field(#s));
                            proxy_index_dispatch::<#ty>(lua, ctx_g.clone(), p)
                        }
                    }
                });
            let proxy_newindex_arms = field_idents
                .iter()
                .zip(field_tys.iter())
                .zip(field_strings.iter())
                .map(|((_id, ty), s)| {
                    quote! {
                        #s => {
                            let mut p = path_s.clone();
                            p.push(::mlua_gen::PathStep::Field(#s));
                            proxy_newindex_dispatch::<#ty>(lua, ctx_s.clone(), p, value)
                        }
                    }
                });
            let build_proxy_arm = quote! {
                #accessor => {
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
                                #(#proxy_index_arms)*
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
                                    #(#proxy_newindex_arms)*
                                    _ => Err(::mlua::Error::runtime(::std::format!(
                                        "no such field: {key_str}"
                                    ))),
                                }
                            })?,
                        )?;
                    }

                    table.set_metatable(Some(mt));
                    Ok(table)
                },
            };

            VariantPieces {
                kind: VariantKind::Named,
                accessor,
                fields_arm,
                project_get_arm,
                project_set_arm,
                build_proxy_arm,
                from_lua_match,
            }
        },

        syn::Fields::Unnamed(field_unnamed) => {
            let arity = field_unnamed.unnamed.len();
            let field_tys: Vec<&syn::Type> = field_unnamed.unnamed.iter().map(|f| &f.ty).collect();
            let field_idents: Vec<syn::Ident> = (0..arity)
                .map(|i| syn::Ident::new(&format!("v{i}"), Span::call_site()))
                .collect();
            let zero_based: Vec<usize> = (0..arity).collect();
            let lua_indices: Vec<usize> = (1..=arity).collect();
            let zero_based_strs: Vec<String> = (0..arity).map(|i| i.to_string()).collect();

            let impl_from_lua = repeat_with(|| {
                quote!(::mlua::FromLua::from_lua(
                    sequence_value.next().ok_or_else(|| {
                        ::mlua::Error::runtime("Not enough values in sequence table.")
                    })??,
                    lua,
                )?)
            })
            .take(arity);
            let from_lua_match = quote! {
                if let Ok(table_value) = table.get::<::mlua::Value>(#accessor) {
                    if let ::mlua::Value::Table(table) = table_value {
                        let mut sequence_value: ::mlua::TableSequence<::mlua::Value> =
                            table.sequence_values();
                        return Ok(Self::#variant_ident( #(#impl_from_lua),* ));
                    }
                }
            };

            let setter_indexed = (1..=arity).map(|i| quote!(table.get(#i)?));
            let fields_arm = quote! {
                reserved_fields.add_field_function_get(
                    #accessor,
                    |lua: &::mlua::Lua, this: ::mlua::AnyUserData| -> ::mlua::Result<::mlua::Value> {
                        let active = ::mlua_gen::with_parent::<Self, _>(&this, |this| {
                            Ok(matches!(this, Self::#variant_ident( .. )))
                        })?;
                        if !active {
                            return Ok(::mlua::Value::Nil);
                        }
                        let on_set: ::std::option::Option<::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>> = #on_set_resolver;
                        let ctx = ::mlua_gen::make_resolver::<Self>(this, on_set);
                        let path = ::std::vec![::mlua_gen::PathStep::Variant(#accessor)];
                        let table = <Self as ::mlua_gen::MluaGenProject>::build_proxy(
                            lua, ctx, path, ::mlua_gen::Visibility::Both,
                        )?;
                        Ok(::mlua::Value::Table(table))
                    },
                );
                reserved_fields.add_field_method_set(
                    #accessor,
                    |_, this, table: ::mlua::Table| {
                        *this = Self::#variant_ident( #(#setter_indexed),* );
                        #on_set_call
                        Ok(())
                    },
                );
            };

            let project_get_field_arms = field_idents
                .iter()
                .zip(field_tys.iter())
                .zip(zero_based.iter())
                .zip(zero_based_strs.iter())
                .map(|(((id, ty), zb), s)| {
                    let body = field_get_body(&quote!((*#id)), ty, s);
                    quote! { ::mlua_gen::PathStep::Tuple(#zb) => { #body } }
                });
            let project_set_field_arms = field_idents
                .iter()
                .zip(field_tys.iter())
                .zip(zero_based.iter())
                .zip(zero_based_strs.iter())
                .map(|(((id, ty), zb), s)| {
                    let body = field_set_body(&quote!((*#id)), ty, s);
                    quote! { ::mlua_gen::PathStep::Tuple(#zb) => { #body } }
                });

            let project_get_arm = quote! {
                #accessor => {
                    let Self::#variant_ident( #(#field_idents),* ) = self else {
                        if rest.is_empty() {
                            return Ok(::mlua::Value::Nil);
                        }
                        return Err(::mlua::Error::runtime(
                            "variant changed under proxy",
                        ));
                    };
                    let Some((step, rest)) = rest.split_first() else {
                        let table = lua.create_table()?;
                        #( let _ = table.push(#field_idents.to_owned()); )*
                        return ::mlua::IntoLua::into_lua(table, lua);
                    };
                    match step {
                        #(#project_get_field_arms)*
                        _ => Err(::mlua_gen::bad_step(#accessor)),
                    }
                },
            };

            let setter_indexed2 = (1..=arity).map(|i| quote!(table.get(#i)?));
            let project_set_arm = quote! {
                #accessor => {
                    if rest.is_empty() {
                        let table: ::mlua::Table =
                            ::mlua::FromLua::from_lua(__mlua_gen_value, lua)?;
                        *self = Self::#variant_ident( #(#setter_indexed2),* );
                        return Ok(());
                    }
                    let Self::#variant_ident( #(#field_idents),* ) = self else {
                        return Err(::mlua::Error::runtime(
                            "variant changed under proxy",
                        ));
                    };
                    let (step, rest) = rest.split_first().expect("checked above");
                    match step {
                        #(#project_set_field_arms)*
                        _ => Err(::mlua_gen::bad_step(#accessor)),
                    }
                },
            };

            let proxy_index_arms = field_tys
                .iter()
                .zip(lua_indices.iter())
                .zip(zero_based.iter())
                .map(|((ty, lua_i), zb)| {
                    quote! {
                        #lua_i => {
                            let mut p = path_g.clone();
                            p.push(::mlua_gen::PathStep::Tuple(#zb));
                            proxy_index_dispatch::<#ty>(lua, ctx_g.clone(), p)
                        }
                    }
                });
            let proxy_newindex_arms = field_tys
                .iter()
                .zip(lua_indices.iter())
                .zip(zero_based.iter())
                .map(|((ty, lua_i), zb)| {
                    quote! {
                        #lua_i => {
                            let mut p = path_s.clone();
                            p.push(::mlua_gen::PathStep::Tuple(#zb));
                            proxy_newindex_dispatch::<#ty>(lua, ctx_s.clone(), p, value)
                        }
                    }
                });
            let build_proxy_arm = quote! {
                #accessor => {
                    let table = lua.create_table()?;
                    let mt = lua.create_table()?;

                    let ctx_g = ctx.clone();
                    let path_g = path.clone();
                    mt.set(
                        "__index",
                        lua.create_function(move |lua, (_, key): (::mlua::Value, usize)| -> ::mlua::Result<::mlua::Value> {
                            match key {
                                #(#proxy_index_arms)*
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
                                    #(#proxy_newindex_arms)*
                                    _ => Err(::mlua::Error::runtime(::std::format!(
                                        "no such tuple field: {key}"
                                    ))),
                                }
                            })?,
                        )?;
                    }

                    table.set_metatable(Some(mt));
                    Ok(table)
                },
            };

            VariantPieces {
                kind: VariantKind::Unnamed,
                accessor,
                fields_arm,
                project_get_arm,
                project_set_arm,
                build_proxy_arm,
                from_lua_match,
            }
        },

        syn::Fields::Unit => {
            let from_lua_match = quote! {
                if let Ok(table_value) = table.get::<::mlua::Value>(#accessor) {
                    if let ::mlua::Value::Boolean(_) = table_value {
                        return Ok(Self::#variant_ident);
                    }
                }
            };
            // Unit variants stay scalar: `true` when active else `nil`. No setter.
            let fields_arm = quote! {
                reserved_fields.add_field_method_get(
                    #accessor,
                    |_, this| -> ::mlua::Result<::std::option::Option<bool>> {
                        Ok(if matches!(this, Self::#variant_ident) {
                            ::std::option::Option::Some(true)
                        } else {
                            ::std::option::Option::None
                        })
                    },
                );
            };
            let project_get_arm = quote! {
                #accessor => {
                    if !rest.is_empty() {
                        return Err(::mlua_gen::bad_step(#accessor));
                    }
                    if matches!(self, Self::#variant_ident) {
                        ::mlua::IntoLua::into_lua(true, lua)
                    } else {
                        Ok(::mlua::Value::Nil)
                    }
                },
            };
            let project_set_arm = quote! {
                #accessor => {
                    if !rest.is_empty() {
                        return Err(::mlua_gen::bad_step(#accessor));
                    }
                    *self = Self::#variant_ident;
                    Ok(())
                },
            };
            let build_proxy_arm = quote! {
                #accessor => {
                    Err(::mlua::Error::runtime(
                        "unit variants are not proxyable (read as bool)",
                    ))
                },
            };

            VariantPieces {
                kind: VariantKind::Unit,
                accessor,
                fields_arm,
                project_get_arm,
                project_set_arm,
                build_proxy_arm,
                from_lua_match,
            }
        },
    }
}
