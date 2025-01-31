use {
    crate::{
        attr::MethodOrFunction,
        builder::{builder_for_fields, builder_for_functions, generate_tuple_access},
        shared::remove_ty_from_generics,
    },
    proc_macro2::{Span, TokenStream as TokenStream2},
    quote::{quote, ToTokens},
    syn::{parse_str, DataStruct, Field, Fields, Generics, Ident},
};

/// Function that impl the `mlua_gen::LuaBuilder` trait for a struct
pub fn builder(
    name: &Ident,
    ds: &DataStruct,
    functions: Vec<&MethodOrFunction>,
    generics: &Generics,
) -> TokenStream2 {
    let builder_code = builder_for_fields(quote! {Self}, &ds.fields);
    let builder_fn_code = builder_for_functions(quote! {Self}, functions);
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

    let table_fn_name = format!("{}_", name);

    quote! {
        impl #generics ::mlua_gen::LuaBuilder<
            #return_type,
            ::mlua::Lua,
            ::mlua::Error,
            ::mlua::Table,
        > for #name #no_ty_generics {
            fn lua_builder(lua: &::mlua::Lua) -> ::mlua::Result<#return_type> {
                #builder_code
            }

            fn lua_fn_builder(lua: &::mlua::Lua) -> ::mlua::Result<Option<::mlua::Table>> {
                #builder_fn_code
            }

            fn to_globals(lua: &::mlua::Lua) -> ::mlua::Result<()> {
                Self::to_globals_as(lua, stringify!(#name))
            }

            fn to_globals_as<S: AsRef<str>>(lua: &::mlua::Lua, s: S) -> ::mlua::Result<()> {
                lua.globals()
                    .set(s.as_ref(), Self::lua_builder(&lua)?)?;

                if let Some(table) = Self::lua_fn_builder(&lua)? {
                    lua.globals().set(#table_fn_name, table)?;
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
    let non_typed_generics = remove_ty_from_generics(generics);

    fn field_accessory_name_and_ident(field: String) -> (TokenStream2, String) {
        let is_int = field.chars().all(|c| c.is_ascii_digit());
        (
            if is_int {
                syn::LitInt::new(&field, Span::call_site()).into_token_stream()
            } else {
                syn::Ident::new(&field, Span::call_site()).into_token_stream()
            },
            if is_int {
                format!(
                    "i{}",
                    field.parse::<usize>().expect(
                        "Already checked that is valid int,\
                                and a tuple shouldn't has usize::MAX fields"
                    ) + 1
                )
            } else {
                field
            },
        )
    }

    // Field
    let (field_get, field_set, field_extra, struct_constructor) = (
        get_fields
            .into_iter()
            .map(|field| {
                let (field_ident, field_accessory_name) = field_accessory_name_and_ident(field);
                quote!(
                    reserved_fields
                        .add_field_method_get(
                            #field_accessory_name,
                            |_, this| Ok(this.#field_ident.clone())
                        );
                )
            })
            .collect::<Vec<TokenStream2>>(),
        set_fields
            .into_iter()
            .map(|field| {
                let (field_ident, field_accessory_name) = field_accessory_name_and_ident(field);

                quote!(
                    reserved_fields
                        .add_field_method_set(
                            #field_accessory_name,
                            |_, this, v| {
                                this.#field_ident = v;
                                Ok(())
                            }
                    );
                )
            })
            .collect::<Vec<TokenStream2>>(),
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
                let impl_from_lua = (0..fields.unnamed.len()).map(|_| {
                    quote!(::mlua::FromLua::from_lua(
                        sequence_value.next().ok_or_else(|| {
                            ::mlua::Error::runtime("Not enough values in sequence table.")
                        })??,
                        lua,
                    )?)
                });

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

    quote! {
        impl #generics ::mlua::FromLua for #name #non_typed_generics {
            fn from_lua(value: ::mlua::Value, lua: &::mlua::Lua) -> ::mlua::Result<#name #non_typed_generics> {
                match value {
                    ::mlua::Value::Table(table) => {
                        Ok(#struct_constructor)
                    },
                    val => Err(::mlua::Error::runtime(format!("Expected a table. Got: {val:?}"))),
                }
            }
        }

        impl #generics ::mlua::UserData for #name #non_typed_generics {
            fn add_fields<MluaUserDataFields: ::mlua::UserDataFields<Self>>(reserved_fields: &mut MluaUserDataFields) {
                #(#field_get)*
                #(#field_set)*
                #field_extra
            }

            fn add_methods<MluaUserDataMethods: ::mlua::UserDataMethods<Self>>(method_or_fns: &mut MluaUserDataMethods) {
                #(#method_or_fns)*
                #method_or_fn_extra
            }
        }
    }
}
