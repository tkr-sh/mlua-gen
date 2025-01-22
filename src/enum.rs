use {
    proc_macro2::Span,
    quote::{quote, ToTokens},
    syn::{Ident, Variant},
};

pub(crate) fn enum_builder<'l, I: Iterator<Item = &'l Variant>>(
    name: &Ident,
    variants: I,
    custom_field: Option<syn::Ident>,
    custom_method_or_fn: Option<syn::Ident>,
) -> proc_macro2::TokenStream {
    // Create the fields to access a value.
    let ((impl_from_lua_match, fields), (fn_constructors, field_constructors)): (
        (Vec<_>, Vec<_>),
        (Vec<_>, Vec<_>),
    ) = variants
        .map(|variant| {
            let original_variant_string = variant.ident.to_string();
            let variant_ident = syn::Ident::new(&original_variant_string, Span::call_site());
            let variant_accessor_name = &original_variant_string.to_lowercase();

            let (match_stmt_get, field_set, constructor, from_lua_impl) = match &variant.fields {
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
                            methods.add_function(
                                #original_variant_string,
                                |_, table: ::mlua::Table| Ok(Self::#variant_ident {
                                    #(#field_constructors)*
                                }),
                            )
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
                            // We name fields `v` + idx
                            let ident = syn::Ident::new(&format!("v{idx}"), Span::call_site());

                            (
                                quote!(let _ = table.push(#ident.to_owned());),
                                quote!(#ident),
                            )
                        })
                        .unzip();

                    // For enum declaration
                    let (argument, indexed, ty) = match field_unnamed.unnamed.len() {
                        0 => (quote!(), quote!(), quote! {()}),
                        // `(my_type)` <=> `my_type` and therefore `.0` won't work
                        1 => {
                            let ty = &field_unnamed.unnamed.first().expect("Length matched").ty;

                            (quote!(param), quote!(value.get(0)?), quote!((#ty)))
                        },
                        _ => {
                            let args_ident = (0..field_unnamed.unnamed.len()).map(|idx| {
                                let index = syn::Index::from(idx);
                                quote!(param.#index)
                            });
                            let args_indexed = (1..=field_unnamed.unnamed.len()).map(|idx| {
                                let index = syn::Index::from(idx);
                                quote!(value.get(#index)?)
                            });

                            let tys = field_unnamed.unnamed.iter().map(|arg| &arg.ty);

                            (
                                quote!(#(#args_ident),*),
                                quote!(#(#args_indexed),*),
                                quote!((#(#tys),*)),
                            )
                        },
                    };

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
                                        #indexed
                                    );
                                    Ok(())
                                }
                            );
                        ),
                        quote!(
                            methods.add_function(
                                #original_variant_string,
                                |_, param: #ty| Ok(Self::#variant_ident(#argument))
                            )
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
                        quote!(
                            reserved_fields.add_field_method_get(
                                #original_variant_string,
                                |_, _| Ok(Self::#variant_ident),
                            )
                        ),
                        quote!(::mlua::Value::Boolean(_) => Some(Self::#variant_ident),),
                    )
                },
            };

            (
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

                        // reserved_fields.add_field_method_set(
                        //     #variant_accessor_name,
                        //     |lua, this, value| Ok(#match_stmt_get)
                        // );
                    ),
                ),
                if matches!(variant.fields, syn::Fields::Unit) {
                    (quote!(), constructor)
                } else {
                    (constructor, quote!())
                },
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
        impl ::mlua::FromLua for #name {
            fn from_lua(value: ::mlua::Value, lua: &::mlua::Lua) -> ::mlua::Result<#name> {
                match value {
                    ::mlua::Value::Table(table) => {
                        #(#impl_from_lua_match)*
                        Err(::mlua::Error::runtime("No valid variant found."))
                    },
                    val => Err(::mlua::Error::runtime(format!("Expected a table. Got: {val:?}"))),
                }
            }
        }

        impl ::mlua::UserData for #name {
            fn add_fields<T: ::mlua::UserDataFields<Self>>(reserved_fields: &mut T) {
                #(#fields)*
                #(#field_constructors);*
                ;
                #extra_fields
            }

            fn add_methods<M: ::mlua::UserDataMethods<Self>>(methods: &mut M) {
                #(#fn_constructors);*
                ;
                #extra_impls
            }
        }
    }
}
