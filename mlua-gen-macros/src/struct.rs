use {
    crate::{
        attr::MethodOrFunction,
        builder::{builder_for_fields, builder_for_functions, generate_tuple_access},
        shared::remove_ty_from_generics,
    },
    proc_macro2::{Span, TokenStream as TokenStream2},
    quote::{ToTokens, quote},
    std::iter::repeat_with,
    syn::{DataStruct, Field, Fields, Generics, Ident, parse_str},
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
    }
}

/// Function that create the user data for a struct
pub(crate) fn user_data(
    name: &Ident,
    generics: &Generics,
    all_fields: &Fields,
    get_fields: Vec<String>,
    set_fields: Vec<String>,
    custom_field: Option<syn::Ident>,
    impls: Vec<MethodOrFunction>,
    custom_method_or_fn: Option<syn::Ident>,
) -> TokenStream2 {
    // Is either a:
    // - When `all_fields` is Named: `Vec` of `.add_method_field_get(...)`
    // - When `all_fields` is Unnamed: `Vec` of `match` arms for __index
    // - When `all_fields` is Unit: `Vec::new`
    let (field_get_named, field_get_unnamed) = match all_fields {
        Fields::Named(_) => {
            (
                get_fields
                    .into_iter()
                    .map(|field| {
                        let field_ident =
                            syn::Ident::new(&field, Span::call_site()).into_token_stream();
                        quote!(
                            reserved_fields
                                .add_field_method_get(
                                    #field,
                                    |_, this| Ok(this.#field_ident.clone())
                                );
                        )
                    })
                    .collect::<Vec<TokenStream2>>(),
                Vec::new(),
            )
        },
        Fields::Unnamed(_) => {
            (
                Vec::new(),
                get_fields
                    .into_iter()
                    .map(|field| {
                        // Since it's an unnamed struct, it should be a usize
                        let field_int =
                            syn::LitInt::new(&field, Span::call_site()).into_token_stream();
                        quote!(
                            #field_int => this.#field_int.clone().into_lua(&lua)?,
                        )
                    })
                    .collect::<Vec<TokenStream2>>(),
            )
        },
        Fields::Unit => (Vec::new(), Vec::new()),
    };

    // Is either a:
    // - When `all_fields` is Named: `Vec` of `.add_method_field_set(...)`
    // - When `all_fields` is Unnamed: `Vec` of `match` arms for __newindex
    // - When `all_fields` is Unit: `Vec::new`
    let (field_set_named, field_set_unnamed) = match all_fields {
        Fields::Named(_) => {
            (
                set_fields
                    .into_iter()
                    .map(|field| {
                        let field_ident =
                            syn::Ident::new(&field, Span::call_site()).into_token_stream();

                        quote!(
                            reserved_fields
                                .add_field_method_set(
                                    #field,
                                    |_, this, v| {
                                        this.#field_ident = v;
                                        Ok(())
                                    }
                                );
                        )
                    })
                    .collect::<Vec<TokenStream2>>(),
                Vec::new(),
            )
        },
        Fields::Unnamed(_) => {
            (
                Vec::new(),
                set_fields
                    .into_iter()
                    .map(|field| {
                        // Since it's an unnamed struct, it should be a usize
                        let field_int =
                            syn::LitInt::new(&field, Span::call_site()).into_token_stream();
                        quote!(
                            #field_int => this.#field_int = mlua::FromLua::from_lua(v, &lua)?,
                        )
                    })
                    .collect::<Vec<TokenStream2>>(),
            )
        },
        Fields::Unit => (Vec::new(), Vec::new()),
    };

    let meta_index = if let Fields::Unnamed(_) = all_fields {
        quote!(
            method_or_fns.add_meta_method("__index", |lua, this, index: usize| {
                use ::mlua::IntoLua;
                Ok(match index - 1 {
                    #(#field_get_unnamed)*
                    _ => return Err::<::mlua::Value, _>(::mlua::Error::runtime(format!("Invalid index: {index}"))),
                })
            });

            method_or_fns.add_meta_method_mut("__newindex", |lua, this: &mut Self, (index, v): (usize, ::mlua::Value)| {
                match index - 1 {
                    #(#field_set_unnamed)*
                    _ => return Err::<(), _>(::mlua::Error::runtime(format!("Invalid index: {index}"))),
                }

                Ok(())
            });
        )
    } else {
        quote!()
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
                #(#field_get_named)*
                #(#field_set_named)*
                #field_extra
            }

            fn add_methods<MluaUserDataMethods: ::mlua::UserDataMethods<Self>>(method_or_fns: &mut MluaUserDataMethods) {
                #meta_index
                #(#method_or_fns)*
                #method_or_fn_extra
            }
        }
    }
}
