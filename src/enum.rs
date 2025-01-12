use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::parse_str;
use syn::Ident;
use syn::Variant;

pub(crate) fn enum_builder<'l, I: Iterator<Item = &'l Variant>>(
    name: &Ident,
    variants: I,
    custom_field: Option<syn::Ident>,
    custom_method_or_fn: Option<syn::Ident>,
) -> proc_macro2::TokenStream {
    // Create the fields to access a value.
    let (fields, (fn_constructors, field_constructors)): (Vec<_>, (Vec<_>, Vec<_>)) = variants
        .map(|variant| {
            let original_variant_string = variant.ident.to_string();
            let variant_ident = syn::Ident::new(&original_variant_string, Span::call_site());
            let variant_accessor_name = &original_variant_string.to_lowercase();

            let (match_stmt, constructor) = match &variant.fields {
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
                    let (set, (fields, field_constructors)): (Vec<_>, (Vec<_>, Vec<_>)) = field_named
                        .named
                        .iter()
                        .map(|named| named.ident.clone().expect("Is named"))
                        .map(|ident| {
                            let stringified_ident = ident.to_token_stream().to_string();
                            (
                                quote!(
                                    let _ = table.set(#stringified_ident, #ident.to_owned());
                                ),
                                (
                                    quote!(#ident),
                                    quote!(
                                        #ident: table.get(#stringified_ident).expect("Missing data"),
                                    ),
                                )
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
                            methods.add_function(
                                #original_variant_string,
                                |_, table: ::mlua::Table| Ok(Self::#variant_ident {
                                    #(#field_constructors)*
                                }),
                            )
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
                    let (set, fields): (Vec<_>, Vec<_>) = (0..field_unnamed.unnamed.len())
                        .map(|idx| {
                            // We name fields `v` + idx
                            let ident = syn::Ident::new(&format!("v{idx}"), Span::call_site());

                            (quote!(let _ = table.push(#ident.to_owned());), quote!(#ident))
                        })
                        .unzip();

                    // For enum declaration
                    let (argument, ty) = match field_unnamed.unnamed.len() {
                        0 => (quote!(), quote! {()}),
                        // `(my_type)` <=> `my_type` and therefore `.0` won't work
                        1 => {
                            let ty = &field_unnamed.unnamed.first().expect("Length matched").ty;

                            (quote!(param), quote!((#ty)))
                        },
                        _ => {
                            let args_ident = (0..field_unnamed.unnamed.len()).into_iter().map(|idx| {
                                let index = syn::Index::from(idx);
                                quote!(param.#index)
                            });

                            let tys = field_unnamed.unnamed.iter().map(|arg| &arg.ty);

                            (quote!(#(#args_ident),*), quote!((#(#tys),*)))
                        },
                    };

                    (
                        quote!(
                            match this {
                                Self::#variant_ident ( #(#fields),* ) => Some({
                                    let mut table = lua.create_table().unwrap();
                                    #(#set)*
                                    table
                                }),
                                _ => None
                            }
                        ),
                        quote!(
                            methods.add_function(
                                #original_variant_string,
                                |_, param: #ty| Ok(Self::#variant_ident(#argument))
                            )
                        )
                    )
                },
                // For enum fields that don't have a value associated to them
                //
                // ```
                // enum A {
                //     Unit,
                // }
                // ```
                syn::Fields::Unit => (
                    quote!(
                        match this { Self::#variant_ident => Some(true), _ => None}
                    ),
                    quote!(
                        fields.add_field_method_get(
                            #original_variant_string,
                            |_, _| Ok(Self::#variant_ident),
                        )
                    ),
                ),
            };



            (
                quote!(
                    fields.add_field_method_get(
                        #variant_accessor_name,
                        |lua, this| Ok(#match_stmt)
                    );
                ),
                if matches!(variant.fields, syn::Fields::Unit) {
                    (quote!(), constructor)

                } else {
                    (constructor, quote!())
                }
            )
        })
        .unzip();
    // .collect::<Vec<TokenStream>>();

    // let method = if let Some(method_or_fn) = custom_method_or_fn {
    //         quote!(#method_or_fn(method_or_fns))
    //     } else {
    //         quote!()
    //     },

    quote! {
        impl ::mlua::UserData for #name {
            fn add_fields<T: ::mlua::UserDataFields<Self>>(fields: &mut T) {
                #(#fields)*
                #(#field_constructors);*
                // #(#field_set)*
                // #field_extra
                // todo!();
            }

            fn add_methods<M: ::mlua::UserDataMethods<Self>>(methods: &mut M) {
                #(#fn_constructors);*
                // #method_or_fn_extra
                // todo!();
            }
        }
    }
}
