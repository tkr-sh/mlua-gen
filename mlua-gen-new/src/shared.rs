use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{punctuated::Punctuated, FieldsUnnamed, Generics};

pub(crate) fn generate_tuple_access(fields_unnamed: FieldsUnnamed) -> (TokenStream2, TokenStream2) {
    match fields_unnamed.unnamed.len() {
        0 => (quote!(), quote! {()}),
        // `(my_type)` <=> `my_type` and therefore `.0` won't work
        1 => {
            let ty = &fields_unnamed.unnamed.first().expect("Length matched").ty;

            (quote!(param), quote!((#ty)))
        },
        _ => {
            let args_ident = (0..fields_unnamed.unnamed.len()).map(|idx| {
                let index = syn::Index::from(idx);
                quote!(param.#index)
            });
            let tys = fields_unnamed.unnamed.iter().map(|arg| &arg.ty);

            (quote!(#(#args_ident),*), quote!((#(#tys),*)))
        },
    }
}

pub(crate) fn remove_ty_from_generics(generics: &Generics) -> TokenStream2 {
    let non_typed_generics_vec = generics
        .params
        .iter()
        .filter_map(|generic| match generic {
            syn::GenericParam::Type(ty) => {
                let mut mut_ty = ty.to_owned();

                mut_ty.colon_token = None;
                mut_ty.bounds = Punctuated::new();
                mut_ty.eq_token = None;
                mut_ty.default = None;

                Some(syn::GenericParam::Type(mut_ty))
            },
            syn::GenericParam::Const(_) => None,
            syn::GenericParam::Lifetime(_) => Some(generic.to_owned()),
        })
        .collect::<Vec<_>>();

    match non_typed_generics_vec.as_slice() {
        &[] => quote!(),
        _ => quote!(<#(#non_typed_generics_vec),*>),
    }
}
