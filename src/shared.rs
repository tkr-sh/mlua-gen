use {
    quote::quote,
    syn::{punctuated::Punctuated, Generics},
};

pub(crate) fn non_typed_generics(generics: &Generics) -> proc_macro2::TokenStream {
    let non_typed_generics_vec = generics
        .params
        .iter()
        .filter_map(|generic| {
            match generic {
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
            }
        })
        .collect::<Vec<_>>();

    match non_typed_generics_vec.as_slice() {
        &[] => quote!(),
        _ => quote!(<#(#non_typed_generics_vec),*>),
    }
}
