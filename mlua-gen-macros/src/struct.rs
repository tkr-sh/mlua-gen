use {
    crate::{
        attr::{MethodOrFunction, MinimalField},
        builder::{builder_for_fields, builder_for_functions, generate_tuple_access},
        project::impl_project,
        shared::remove_ty_from_generics,
    },
    proc_macro2::{Span, TokenStream as TokenStream2},
    quote::{ToTokens, quote},
    std::{collections::HashSet, iter::repeat_with},
    syn::{DataStruct, Field, Fields, Generics, Ident, Path, parse_str},
};

/// Function that impl the `mlua_gen::LuaBuilder` trait for a struct
pub fn builder(
    name: &Ident,
    ds: &DataStruct,
    functions: Vec<&MethodOrFunction>,
    generics: &Generics,
) -> TokenStream2 {
    let init_builder_code = builder_for_fields(&quote! {Self}, &ds.fields, false);
    let maybe_set_metatable = if ds.fields == Fields::Unit {
        quote!()
    } else {
        let function_wrap_builder_code = builder_for_fields(&quote! {Self}, &ds.fields, true);

        quote! {
            table.set_metatable(Some({
                let metatable = lua.create_table()?;
                metatable.set(
                    "__call",
                    #function_wrap_builder_code,
                )?;
                metatable
            }));
        }
    };
    let builder_fn_code = builder_for_functions(&quote! {Self}, functions);
    let no_ty_generics = remove_ty_from_generics(generics);

    // The reason for that is that, when we have a unit struct, we just want to be able to call it
    // like normal:
    // ```
    // local cat = Cat
    // ```
    // without passing any argument. While, when it's a unnamed | named, we want to call a
    // function:
    // ```
    // local cat = Cat ( "nyan" )
    // local cat = Cat { name = "nyan" }
    // ```
    let return_type = match ds.fields {
        Fields::Unit => quote!(Self),
        Fields::Unnamed(..) | Fields::Named(..) => quote!(::mlua::Function),
    };

    quote! {
        impl #generics ::mlua_gen::LuaBuilder<
            #return_type,
            ::mlua::Lua,
            ::mlua::Error,
            ::mlua::Table,
        > for #name #no_ty_generics {
            fn lua_builder(lua: &::mlua::Lua) -> ::mlua::Result<#return_type> {
                #init_builder_code
            }

            fn lua_fn_builder(lua: &::mlua::Lua) -> ::mlua::Result<Option<::mlua::Table>> {
                #builder_fn_code
            }

            fn to_globals(lua: &::mlua::Lua) -> ::mlua::Result<()> {
                Self::to_globals_as(lua, stringify!(#name))
            }

            fn to_globals_as<S: AsRef<str>>(lua: &::mlua::Lua, s: S) -> ::mlua::Result<()> {
                // When there are no function constructors (`new`, `default`, etc.), we can just
                // put it as a basic `function` (Cf. `else` block). But when it's not, we need to
                // create a metatable just for that.
                if let Some(table) = Self::lua_fn_builder(&lua)? {
                    #maybe_set_metatable

                    lua.globals().set(s.as_ref(), table)?;
                } else {
                    lua.globals()
                        .set(s.as_ref(), Self::lua_builder(&lua)?)?;
                }

                Ok(())
            }
        }

        impl #generics ::mlua_gen::AutomaticImplWhenMluaGen for #name #no_ty_generics { }
    }
}

/// Function that create the user data for a struct
pub(crate) fn user_data(
    name: &Ident,
    generics: &Generics,
    all_fields: &Fields,
    get_fields: Vec<MinimalField>,
    set_fields: Vec<MinimalField>,
    custom_field: Option<syn::Ident>,
    impls: Vec<MethodOrFunction>,
    custom_method_or_fn: Option<syn::Ident>,
    on_set: Option<Path>,
) -> TokenStream2 {
    let on_set_call = match &on_set {
        Some(path) => quote!( (#path)(); ),
        None => quote!(),
    };
    let on_set_resolver = match &on_set {
        Some(path) => {
            quote! {
                ::std::option::Option::Some(::std::sync::Arc::new(|| { (#path)(); }) as ::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>)
            }
        },
        None => quote!(::std::option::Option::None),
    };
    let (fields_declaration, meta_index) = match all_fields {
        Fields::Named(_) => {
            let get_and_set_fields = get_fields
                .iter()
                .chain(set_fields.iter())
                .collect::<HashSet<&MinimalField>>();
            let mut fields_code = vec![];

            for get_and_set_field in get_and_set_fields {
                let is_get = get_fields.contains(get_and_set_field);
                let is_set = set_fields.contains(get_and_set_field);
                let field = get_and_set_field;
                let field_ident = &field.ident;
                let field_as_string = &field.ident_string;
                let field_ty = &field.ty;

                let base_code = quote!(
                    match (
                        <#field_ty as ::mlua_gen::IsMluaGenerated>::IS_MLUA_GENERATED,
                        <#field_ty as ::mlua_gen::CollectionProject>::IS_COLLECTION_OF_MLUA_GEN,
                        <#field_ty as ::mlua_gen::IsIndexable>::IS_INDEXABLE,
                    ) {
                        // Nested `#[mlua_gen]` field — recurse via build_proxy.
                        (true, _, _) => {
                            if #is_get {
                                reserved_fields.add_field_function_get(#field_as_string, |lua: &::mlua::Lua, this: ::mlua::AnyUserData| {
                                    let on_set: ::std::option::Option<::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>> = #on_set_resolver;
                                    let ctx = ::mlua_gen::make_resolver::<Self>(this, on_set);
                                    let path = ::std::vec![::mlua_gen::PathStep::Field(#field_as_string)];
                                    let vis = if #is_set {
                                        ::mlua_gen::Visibility::Both
                                    } else {
                                        ::mlua_gen::Visibility::GetOnly
                                    };
                                    let table = <#field_ty as ::mlua_gen::MluaGenProjectMaybe>::maybe_build_proxy(lua, ctx, path, vis)?;
                                    Ok(::mlua::Value::Table(table))
                                });
                            }
                            // Whole-field replacement: outer.inner = { ... }
                            if #is_set {
                                reserved_fields.add_field_method_set(
                                    #field_as_string,
                                    |_, this, v: #field_ty| {
                                        this.#field_ident = v;
                                        #on_set_call
                                        Ok(())
                                    }
                                );
                            }
                        }
                        // Collection of `mlua_gen` elements.
                        (_, true, _) => {
                            if #is_get {
                                reserved_fields.add_field_function_get(#field_as_string, |lua: &::mlua::Lua, this: ::mlua::AnyUserData| {
                                    let on_set: ::std::option::Option<::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>> = #on_set_resolver;
                                    let ctx = ::mlua_gen::make_resolver::<Self>(this, on_set);
                                    let path = ::std::vec![::mlua_gen::PathStep::Field(#field_as_string)];
                                    let vis = if #is_set {
                                        ::mlua_gen::Visibility::Both
                                    } else {
                                        ::mlua_gen::Visibility::GetOnly
                                    };
                                    let table = <#field_ty as ::mlua_gen::CollectionProject>::build_collection_proxy(lua, ctx, path, vis)?;
                                    Ok(::mlua::Value::Table(table))
                                });
                            }
                            if #is_set {
                                reserved_fields.add_field_method_set(
                                    #field_as_string,
                                    |_, this, v: #field_ty| {
                                        this.#field_ident = v;
                                        #on_set_call
                                        Ok(())
                                    }
                                );
                            }
                        }
                        // Indexable collection of leaf values.
                        (_, _, true) => {
                            if #is_get {
                                reserved_fields.add_field_function_get(#field_as_string, |lua: &::mlua::Lua, this: ::mlua::AnyUserData| {
                                    let on_set: ::std::option::Option<::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>> = #on_set_resolver;
                                    let ctx = ::mlua_gen::make_resolver::<Self>(this, on_set);
                                    let path = ::std::vec![::mlua_gen::PathStep::Field(#field_as_string)];
                                    let vis = if #is_set {
                                        ::mlua_gen::Visibility::Both
                                    } else {
                                        ::mlua_gen::Visibility::GetOnly
                                    };
                                    let table = ::mlua_gen::build_indexed_proxy_leaf(lua, ctx, path, vis)?;
                                    Ok(::mlua::Value::Table(table))
                                });
                            }
                            if #is_set {
                                reserved_fields.add_field_method_set(
                                    #field_as_string,
                                    |_, this, v: #field_ty| {
                                        this.#field_ident = v;
                                        #on_set_call
                                        Ok(())
                                    }
                                );
                            }
                        }
                        // Leaf — clone in/out as before.
                        (false, false, false) => {
                            if #is_get {
                                reserved_fields
                                    .add_field_method_get(
                                        #field_as_string,
                                        |_, this| Ok(this.#field_ident.clone())
                                    );
                            }

                            if #is_set {
                                reserved_fields
                                    .add_field_method_set(
                                        #field_as_string,
                                        |_, this, v| {
                                            this.#field_ident = v;
                                            #on_set_call
                                            Ok(())
                                        }
                                    );
                            }
                        }
                    }
                );


                fields_code.push(base_code);
            }

            (fields_code, quote!())
        },
        Fields::Unnamed(_) => {
            // Tuple struct: `__index` / `__newindex` meta-functions on the
            // struct, with the same `(IS_MLUA_GENERATED, IS_INDEXABLE)` arms
            // as named structs. Lua is 1-based; conversion is at codegen.
            let set_field_strings: HashSet<&String> =
                set_fields.iter().map(|f| &f.ident_string).collect();
            let get_arms = get_fields.iter().map(|field| {
                let ident = &field.ident;
                let ty = &field.ty;
                let zero_based: usize = field.ident_string.parse::<usize>().unwrap();
                let lua_index: usize = zero_based + 1;
                let is_set = set_field_strings.contains(&field.ident_string);
                quote! {
                    #lua_index => {
                        let on_set: ::std::option::Option<::std::sync::Arc<dyn Fn() + ::std::marker::Send + ::std::marker::Sync>> = #on_set_resolver;
                        let ctx = ::mlua_gen::make_resolver::<Self>(this.clone(), on_set);
                        let path = ::std::vec![::mlua_gen::PathStep::Tuple(#zero_based)];
                        let vis = if #is_set {
                            ::mlua_gen::Visibility::Both
                        } else {
                            ::mlua_gen::Visibility::GetOnly
                        };
                        match (
                            <#ty as ::mlua_gen::IsMluaGenerated>::IS_MLUA_GENERATED,
                            <#ty as ::mlua_gen::CollectionProject>::IS_COLLECTION_OF_MLUA_GEN,
                            <#ty as ::mlua_gen::IsIndexable>::IS_INDEXABLE,
                        ) {
                            (true, _, _) => {
                                let table = <#ty as ::mlua_gen::MluaGenProjectMaybe>::maybe_build_proxy(lua, ctx, path, vis)?;
                                ::mlua::Value::Table(table)
                            },
                            (_, true, _) => {
                                let table = <#ty as ::mlua_gen::CollectionProject>::build_collection_proxy(lua, ctx, path, vis)?;
                                ::mlua::Value::Table(table)
                            },
                            (_, _, true) => {
                                let table = ::mlua_gen::build_indexed_proxy_leaf(lua, ctx, path, vis)?;
                                ::mlua::Value::Table(table)
                            },
                            (false, false, false) => {
                                ::mlua_gen::with_parent::<Self, _>(&this, |this| {
                                    use ::mlua::IntoLua;
                                    this.#ident.clone().into_lua(lua)
                                })?
                            },
                        }
                    },
                }
            });

            let set_arms = set_fields.iter().map(|field| {
                let ident = &field.ident;
                let ty = &field.ty;
                let lua_index: usize = field.ident_string.parse::<usize>().unwrap() + 1;
                quote! {
                    #lua_index => {
                        let v = <#ty as ::mlua::FromLua>::from_lua(v, lua)?;
                        ::mlua_gen::with_parent_mut::<Self, _>(&this, |this| {
                            this.#ident = v;
                            Ok(())
                        })?;
                        #on_set_call
                    },
                }
            });

            let meta = quote! {
                method_or_fns.add_meta_function("__index", |lua, (this, index): (::mlua::AnyUserData, usize)| {
                    use ::mlua::IntoLua;
                    Ok::<::mlua::Value, ::mlua::Error>(match index {
                        #(#get_arms)*
                        _ => return Err(::mlua::Error::runtime(
                            format!("Invalid index: {index}")
                        )),
                    })
                });

                method_or_fns.add_meta_function(
                    "__newindex",
                    |lua, (this, index, v): (::mlua::AnyUserData, usize, ::mlua::Value)| {
                        match index {
                            #(#set_arms)*
                            _ => return Err::<(), _>(::mlua::Error::runtime(
                                format!("Invalid index: {index}")
                            )),
                        }
                        Ok(())
                    },
                );
            };

            (vec![], meta)
        },
        Fields::Unit => (vec![], quote!()),
    };

    // Field
    let (field_extra, struct_constructor) = (
        if let Some(field) = custom_field {
            quote! {#field(reserved_fields)}
        } else {
            quote!()
        },
        // Code to `impl FromLua`
        match all_fields {
            Fields::Named(fields) => {
                let named_fields_constructor = fields
                    .named
                    .iter()
                    .map(|named| named.ident.as_ref().expect("Is named"))
                    .map(|field| {
                        let stringified_field = field.to_token_stream().to_string();
                        quote!(#field: table.get(#stringified_field)?)
                    });

                quote!(Self {
                    #(#named_fields_constructor),*
                })
            },
            Fields::Unnamed(fields) => {
                // For impl from lua
                let impl_from_lua = repeat_with(|| {
                    quote!(::mlua::FromLua::from_lua(
                        sequence_value.next().ok_or_else(|| {
                            ::mlua::Error::runtime("Not enough values in sequence table.")
                        })??,
                        lua,
                    )?)
                })
                .take(fields.unnamed.len());

                quote!(
                    {
                        let mut sequence_value: ::mlua::TableSequence<::mlua::Value> =
                            table.sequence_values();

                        Self(
                            #(#impl_from_lua),*
                        )
                    }
                )
            },
            Fields::Unit => quote!(Self),
        },
    );



    let (method_or_fns, method_or_fn_extra) = (
        impls
            .into_iter()
            .map(|method_or_fn| {
                let method_or_fn_ident = syn::Ident::new(&method_or_fn.name, Span::call_site());
                let method_or_fn_string = method_or_fn.name;

                let owned_fields: Vec<_> = method_or_fn
                    .args
                    .iter()
                    .map(|arg| {
                        Field {
                            colon_token: None,
                            attrs:       vec![],
                            vis:         syn::Visibility::Inherited,
                            mutability:  syn::FieldMutability::None,
                            ident:       None,
                            ty:          parse_str::<syn::Type>(arg).expect("Invalid type."),
                        }
                    })
                    .collect();
                let (argument, ty) = generate_tuple_access(owned_fields.iter());

                let add_kind = match (method_or_fn.is_mut, method_or_fn.is_self) {
                    (true, true) => quote!(add_method_mut),
                    (false, true) => quote!(add_method),
                    (true, false) => quote!(add_function_mut),
                    (false, false) => quote!(add_function),
                };

                let (method_or_fn_caller, this) = if method_or_fn.is_self {
                    (quote!(this.), quote!(this,))
                } else {
                    (quote!(Self::), quote!())
                };

                quote!(
                    method_or_fns.#add_kind(#method_or_fn_string, |_, #this args: #ty| {
                        Ok(#method_or_fn_caller #method_or_fn_ident(#argument))
                    });
                )
            })
            .collect::<Vec<TokenStream2>>(),
        if let Some(method_or_fn) = custom_method_or_fn {
            quote!(#method_or_fn(method_or_fns))
        } else {
            quote!()
        },
    );



    let non_typed_generics = remove_ty_from_generics(generics);

    let project_impl = impl_project(name, generics, all_fields, &get_fields, &set_fields);

    quote! {
        #project_impl

        impl #generics ::mlua::FromLua for #name #non_typed_generics {
            fn from_lua(value: ::mlua::Value, lua: &::mlua::Lua) -> ::mlua::Result<#name #non_typed_generics> {
                match value {
                    ::mlua::Value::Table(table) => {
                        Ok(#struct_constructor)
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
                // #(#field_get_named)*
                // #(#field_set_named)*
                #(#fields_declaration)*
                #field_extra
            }

            fn add_methods<MluaUserDataMethods: ::mlua::UserDataMethods<Self>>(method_or_fns: &mut MluaUserDataMethods) {
                #(#method_or_fns)*
                #meta_index
                #method_or_fn_extra
            }
        }
    }
}
