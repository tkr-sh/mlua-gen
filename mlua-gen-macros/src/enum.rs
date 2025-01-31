use {
    crate::{
        attr::MethodOrFunction,
        builder::{builder_for_fields, builder_for_functions},
        shared::remove_ty_from_generics,
    },
    proc_macro2::{Span, TokenStream as TokenStream2},
    quote::{quote, ToTokens},
    syn::{DataEnum, Generics, Ident, Variant},
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
                builder_for_fields(quote! {Self::#var_name}, &v.fields),
            )
        })
        .unzip();
    let no_ty_generics = remove_ty_from_generics(generics);

    let builder_fn_code = builder_for_functions(quote! {Self}, functions);

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
    }
}

pub(crate) fn user_data<'l, I: Iterator<Item = &'l Variant>>(
    name: &Ident,
    generics: &Generics,
    variants: I,
    custom_field: Option<syn::Ident>,
    custom_method_or_fn: Option<syn::Ident>,
) -> proc_macro2::TokenStream {
    let non_typed_generics = remove_ty_from_generics(generics);

    // Create the fields to access a value.
    let (impl_from_lua_match, fields): (Vec<_>, Vec<_>) = variants
        .map(|variant| {
            let original_variant_string = variant.ident.to_string();
            let variant_ident = syn::Ident::new(&original_variant_string, Span::call_site());
            let variant_accessor_name = &original_variant_string.to_lowercase();

            let (match_stmt_get, field_set, from_lua_impl) = match &variant.fields {
                // For enum fields that are named
                //
                // ```
                // enum A {
                //     Named {
                //          name: String,
                //     }
                // }
                // ```
                syn::Fields::Named(field_named) => {
                    let (set, (fields, field_constructors)): (Vec<_>, (Vec<_>, Vec<_>)) =
                        field_named
                            .named
                            .iter()
                            .map(|named| named.ident.as_ref().expect("Is named"))
                            .map(|ident| {
                                let stringified_ident = ident.to_token_stream().to_string();
                                (
                                    quote!(
                                        let _ = table.set(#stringified_ident, #ident.to_owned());
                                    ),
                                    (
                                        quote!(#ident),
                                        quote!(
                                            #ident: table.get(#stringified_ident)?,
                                        ),
                                    ),
                                )
                            })
                            .unzip();

                    (
                        quote!(
                            match this {
                                Self::#variant_ident { #(#fields),* } => Some({
                                    let mut table = lua.create_table().unwrap();
                                    #(#set)*
                                    table
                                }),
                                _ => None
                            }
                        ),
                        quote!(
                            reserved_fields.add_field_method_set(
                                #variant_accessor_name,
                                |lua, this, table: ::mlua::Table| {
                                    *this = Self::#variant_ident {
                                        #(#field_constructors)*
                                    };
                                    Ok(())
                                }
                            );
                        ),
                        quote!(
                            ::mlua::Value::Table(table) => {
                                Some(Self::#variant_ident {
                                    #(#field_constructors)*
                                })
                            },
                        ),
                    )
                },
                // For enum fields that don't have a name / tuple
                //
                // ```
                // enum A {
                //     Unnamed(String),
                // }
                // ```
                syn::Fields::Unnamed(field_unnamed) => {
                    // For field access
                    let (set, field_idents): (Vec<_>, Vec<_>) = (0..field_unnamed.unnamed.len())
                        .map(|idx| {
                            // We name fields `v` + idx only for deconstruction
                            let ident = syn::Ident::new(&format!("v{idx}"), Span::call_site());

                            (
                                quote!(let _ = table.push(#ident.to_owned());),
                                quote!(#ident),
                            )
                        })
                        .unzip();

                    let indexed = (1..=field_unnamed.unnamed.len()).map(|i| quote!(value.get(#i)?));

                    // For impl from lua
                    let impl_from_lua = (0..field_unnamed.unnamed.len()).map(|_| {
                        quote!(::mlua::FromLua::from_lua(
                            sequence_value.next().ok_or_else(|| {
                                ::mlua::Error::runtime("Not enough values in sequence table.")
                            })??,
                            lua,
                        )?)
                    });

                    (
                        quote!(
                            match this {
                                Self::#variant_ident ( #(#field_idents),* ) => Some({
                                    let mut table = lua.create_table().unwrap();
                                    #(#set)*
                                    table
                                }),
                                _ => None
                            }
                        ),
                        quote!(
                            reserved_fields.add_field_method_set(
                                #variant_accessor_name,
                                |lua, this, value: ::mlua::Table| {
                                    *this = Self::#variant_ident(
                                        #(#indexed),*
                                    );
                                    Ok(())
                                }
                            );
                        ),
                        quote!(
                            ::mlua::Value::Table(table) => {
                                let mut sequence_value: ::mlua::TableSequence<::mlua::Value> =
                                    table.sequence_values();
                                Some(Self::#variant_ident(
                                    #(#impl_from_lua),*
                                ))
                            },
                        ),
                    )
                },
                // For enum fields that don't have a value associated to them
                //
                // ```
                // enum A {
                //     Unit,
                // }
                // ```
                syn::Fields::Unit => {
                    (
                        quote!(
                            match this { Self::#variant_ident => Some(true), _ => None}
                        ),
                        quote!(),
                        quote!(::mlua::Value::Boolean(_) => Some(Self::#variant_ident),),
                    )
                },
            };

            (
                quote!(
                    if let Ok(table_value) = table.get(#variant_accessor_name) {
                        if let Some(value) = match table_value {
                            #from_lua_impl
                            ::mlua::Value::Nil => None,
                            _ => return Err(::mlua::Error::runtime("Invalid data format.")),
                        } {
                            return Ok(value);
                        }
                    }
                ),
                quote!(
                    reserved_fields.add_field_method_get(
                        #variant_accessor_name,
                        |lua, this| Ok(#match_stmt_get)
                    );
                    #field_set
                ),
            )
        })
        .unzip();

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

    quote! {
        impl #generics ::mlua::FromLua for #name #non_typed_generics{
            fn from_lua(value: ::mlua::Value, lua: &::mlua::Lua) -> ::mlua::Result<#name #non_typed_generics> {
                match value {
                    ::mlua::Value::Table(table) => {
                        #(#impl_from_lua_match)*
                        Err(::mlua::Error::runtime("No valid variant found."))
                    },
                    val => Err(::mlua::Error::runtime(format!("Expected a table. Got: {val:?}"))),
                }
            }
        }

        impl #generics ::mlua::UserData for #name #non_typed_generics {
            fn add_fields<MluaUserDataFields: ::mlua::UserDataFields<Self>>(reserved_fields: &mut MluaUserDataFields) {
                #(#fields)*
                ;
                #extra_fields
            }

            fn add_methods<MluaUserDataMethods: ::mlua::UserDataMethods<Self>>(methods: &mut MluaUserDataMethods) {
                #extra_impls
            }
        }
    }
}
