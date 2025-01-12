use {
    crate::MethodOrFunction,
    proc_macro2::{Span, TokenStream},
    quote::quote,
    syn::{parse_str, Ident},
};

pub(crate) fn struct_builder(
    name: &Ident,
    get_fields: Vec<String>,
    set_fields: Vec<String>,
    custom_field: Option<syn::Ident>,
    impls: Vec<MethodOrFunction>,
    custom_method_or_fn: Option<syn::Ident>,
) -> TokenStream {
    // Field
    let (field_get, field_set, field_extra) =
        (
            get_fields
                .into_iter()
                .map(|field| {
                    let field_ident = syn::Ident::new(&field, Span::call_site());
                    quote!(reserved_fields.add_field_method_get(#field, |_, this| Ok(this.#field_ident.clone()));)
                })
                .collect::<Vec<TokenStream>>(),
            set_fields
                .into_iter()
                .map(|field| {
                    let field_ident = syn::Ident::new(&field, Span::call_site());
                    quote!(
                        reserved_fields.add_field_method_set(#field, |_, this, v| {
                            this.#field_ident = v;
                            Ok(())
                        });
                    )
                })
                .collect::<Vec<TokenStream>>(),
            if let Some(field) = custom_field {
                quote!{#field(reserved_fields)}
            } else {
                quote!()
            }
        );

    let (method_or_fns, method_or_fn_extra) = (
        impls
            .into_iter()
            .map(|method_or_fn| {
                let method_or_fn_ident = syn::Ident::new(&method_or_fn.name, Span::call_site());
                let method_or_fn_string = method_or_fn.name;

                let (argument, ty) = match method_or_fn.args.len() {
                    0 => (quote!(), quote! {()}),
                    // `(my_type)` <=> `my_type` and therefore `.0` won't work
                    1 => {
                        let ty = parse_str::<syn::Type>(
                            method_or_fn.args.first().expect("Length matched"),
                        )
                        .unwrap();

                        (quote!(param), quote!((#ty)))
                    },
                    _ => {
                        let args_ident = (0..method_or_fn.args.len()).map(|idx| {
                            let index = syn::Index::from(idx);
                            quote!(param.#index)
                        });

                        let args_ty = method_or_fn
                            .args
                            .iter()
                            .map(|arg| parse_str::<syn::Type>(arg).unwrap());

                        (quote!(#(#args_ident),*), quote!((#(#args_ty),*)))
                    },
                };

                let add_kind = match (method_or_fn.is_mut, method_or_fn.is_method) {
                    (true, true) => quote!(add_method_mut),
                    (false, true) => quote!(add_method),
                    (true, false) => quote!(add_function_mut),
                    (false, false) => quote!(add_function),
                };

                let (method_or_fn_caller, this) = if method_or_fn.is_method {
                    (quote!(this.), quote!(this,))
                } else {
                    (quote!(Self::), quote!())
                };

                quote!(
                    method_or_fns.#add_kind(#method_or_fn_string, |_, #this param: #ty| {
                        Ok(#method_or_fn_caller #method_or_fn_ident(#argument))
                    });
                )
            })
            .collect::<Vec<TokenStream>>(),
        if let Some(method_or_fn) = custom_method_or_fn {
            quote!(#method_or_fn(method_or_fns))
        } else {
            quote!()
        },
    );

    quote! {
        impl ::mlua::UserData for #name {
            fn add_fields<T: ::mlua::UserDataFields<Self>>(reserved_fields: &mut T) {
                #(#field_get)*
                #(#field_set)*
                #field_extra
            }

            fn add_methods<M: ::mlua::UserDataMethods<Self>>(method_or_fns: &mut M) {
                #(#method_or_fns)*
                #method_or_fn_extra
            }
        }
    }
}
