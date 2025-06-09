use {
    crate::{
        attr::{MethodOrFunction, MinimalField},
        builder::{builder_for_fields, builder_for_functions, generate_tuple_access},
        shared::remove_ty_from_generics,
    },
    proc_macro2::{Span, TokenStream as TokenStream2},
    quote::{quote, ToTokens},
    std::{collections::HashSet, iter::repeat_with},
    syn::{parse_str, DataStruct, Field, Fields, Generics, Ident},
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
) -> TokenStream2 {
    let fields_declaration = match all_fields {
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
                    // TODO: based on trait const boolean either:
                    //
                    // - add_field_function_get
                    // - add_field_method_get => for non indexable and into_lua

                    match (<#field_ty as ::mlua_gen::IsMluaGenerated>::IS_MLUA_GENERATED, <#field_ty as ::mlua_gen::IsIndexable>::IS_INDEXABLE) {
                        (true, _) => {
                            reserved_fields.add_field_function_get(#field_as_string, |lua: &::mlua::Lua, this: ::mlua::AnyUserData| {
                                use ::mlua::IntoLua;
                                let table = lua.create_table()?;
                                let this_clone = this.clone();
                                let mut meta_table = vec![];

                                if #is_get {
                                    meta_table.push(
                                        (
                                            "__index",
                                            lua.create_function(move |_, (_, index): (::mlua::Table, usize)| {
                                                let this = this.borrow::<::std::sync::Arc<::std::sync::Mutex<Self>>>()?;
                                                Ok(this.lock().unwrap().#field_ident.clone())
                                            })?
                                        )
                                    );
                                }

                                if #is_set {
                                    meta_table.push(
                                        (
                                            "__newindex",
                                            lua.create_function(
                                                move |lua, (_, key, value): (::mlua::Table, String, ::mlua::Value)| {
                                                    use ::mlua::IntoLua;
                                                    let mut this = this_clone.borrow_mut::<::std::sync::Arc<::std::sync::Mutex<Self>>>()?;
                                                    let ::mlua::Value::UserData(data) = this.lock().unwrap().#field_ident.clone().into_lua(lua)? else {
                                                        unreachable!("Conversion of types that come from mlua-gen should ALWAYS be converted to UserData")
                                                    };

                                                    use ::mlua::ObjectLike;
                                                    data.set(key.clone(), value)?;

                                                    this.lock().unwrap().#field_ident = ::mlua::FromLua::from_lua(::mlua::Value::UserData(data), lua)?;
                                                    Ok(())
                                                },
                                            )?
                                        )
                                    );
                                }

                                table.set_metatable(Some(lua.create_table_from(meta_table)?));

                                table.into_lua(lua)
                            });
                        }
                        (false, true) => {
                            reserved_fields.add_field_function_get(#field_as_string, |lua: &::mlua::Lua, this: ::mlua::AnyUserData| {
                                use ::mlua::IntoLua;

                                let table = lua.create_table()?;
                                let this_clone = this.clone();
                                // TODO get/set checks
                                //meanwhlie:

                                let mut meta_table = vec![
                                    (
                                        "__index",
                                        lua.create_function(move |_, (_, index): (::mlua::Table, usize)| {
                                            use mlua_gen::IsIndexable;
                                            let this = this.borrow::<::std::sync::Arc<::std::sync::Mutex<Self>>>()?;
                                            Ok(this.lock().unwrap().#field_ident.index_or_unreachable(index - 1))
                                        })?
                                    )
                                ];

                                if <#field_ty as ::mlua_gen::IsNewIndexable>::IS_NEW_INDEXABLE {
                                    use ::mlua_gen::IsNewIndexable;
                                    meta_table.push(
                                        (
                                            "__newindex",
                                            lua.create_function(move |_, (_, index, value): (::mlua::Table, <#field_ty as IsNewIndexable>::Key, <#field_ty as IsNewIndexable>::Item )| {
                                                let mut this = this_clone.borrow_mut::<::std::sync::Arc<::std::sync::Mutex<Self>>>()?;
                                                this.lock().unwrap().#field_ident.set_index_or_unreachable(index, value);
                                                Ok(())
                                            })?
                                        )
                                    );
                                }

                                let mt = lua.create_table_from(meta_table)?;
                                table.set_metatable(Some(mt));
                                table.into_lua(lua)

                            });
                        }
                        (false, false) => {
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
                                            Ok(())
                                        }
                                    );
                            }
                        }
                    }
                );


                fields_code.push(base_code);
            }

            fields_code
        },
        Fields::Unnamed(_) | Fields::Unit => {
            // TODO:
            vec![]
        },
    };
    //
    // // }
    // let (field_get_named, field_get_unnamed) = match all_fields {
    //     Fields::Named(_) => {
    //         (
    //             get_fields
    //                 .into_iter()
    //                 .map(|field| {
    //                     let field_ident = field.ident;
    //                     let field_ident_string = field.ident_string;
    //                     quote!(
    //                         reserved_fields
    //                             .add_field_method_get(
    //                                 #field_ident_string,
    //                                 |_, this| Ok(this.#field_ident.clone())
    //                             );
    //                     )
    //                 })
    //                 .collect::<Vec<TokenStream2>>(),
    //             Vec::new(),
    //         )
    //     },
    //     Fields::Unnamed(_) => {
    //         (
    //             Vec::new(),
    //             get_fields
    //                 .into_iter()
    //                 .map(|field| {
    //                     // Since it's an unnamed struct, it should be a usize
    //                     let field_int = field.ident;
    //                     quote!(
    //                         #field_int => this.#field_int.clone().into_lua(&lua)?,
    //                     )
    //                 })
    //                 .collect::<Vec<TokenStream2>>(),
    //         )
    //     },
    //     Fields::Unit => (Vec::new(), Vec::new()),
    // };
    //
    // // Is either a:
    // // - When `all_fields` is Named: `Vec` of `.add_method_field_set(...)`
    // // - When `all_fields` is Unnamed: `Vec` of `match` arms for __newindex
    // // - When `all_fields` is Unit: `Vec::new`
    // let (field_set_named, field_set_unnamed) = match all_fields {
    //     Fields::Named(_) => {
    //         (
    //             set_fields
    //                 .into_iter()
    //                 .map(|field| {
    //                     let field_ident = field.ident;
    //                     let field_ident_string = field.ident_string;
    //
    //                     quote!(
    //                         reserved_fields
    //                             .add_field_method_set(
    //                                 #field_ident_string,
    //                                 |_, this, v| {
    //                                     this.#field_ident = v;
    //                                     Ok(())
    //                                 }
    //                             );
    //                     )
    //                 })
    //                 .collect::<Vec<TokenStream2>>(),
    //             Vec::new(),
    //         )
    //     },
    //     Fields::Unnamed(_) => {
    //         (
    //             Vec::new(),
    //             set_fields
    //                 .into_iter()
    //                 .map(|field| {
    //                     // Since it's an unnamed struct, it should be a usize
    //                     let field_int = field.ident;
    //                     quote!(
    //                         #field_int => this.#field_int = mlua::FromLua::from_lua(v, &lua)?,
    //                     )
    //                 })
    //                 .collect::<Vec<TokenStream2>>(),
    //         )
    //     },
    //     Fields::Unit => (Vec::new(), Vec::new()),
    // };



    // let meta_index = if matches!(all_fields, Fields::Unnamed(_)) {
    //     quote!(
    //         method_or_fns.add_meta_method("__index", |lua, this, index: usize| {
    //             use ::mlua::IntoLua;
    //             Ok(match index - 1 {
    //                 #(#field_get_unnamed)*
    //                 _ => return Err::<::mlua::Value, _>(::mlua::Error::runtime(format!("Invalid index: {index}"))),
    //             })
    //         });
    //
    //         method_or_fns.add_meta_method_mut("__newindex", |lua, this: &mut Self, (index, v): (usize, ::mlua::Value)| {
    //             match index - 1 {
    //                 #(#field_set_unnamed)*
    //                 _ => return Err::<(), _>(::mlua::Error::runtime(format!("Invalid index: {index}"))),
    //             }
    //
    //             Ok(())
    //         });
    //     )
    // } else {
    //     quote!()
    // };



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

    quote! {
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
                #method_or_fn_extra
            }
        }
    }
}
