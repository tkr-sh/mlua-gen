use {
    attr::Attributes,
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    std::collections::HashMap,
    syn::{parse_macro_input, Data, DeriveInput},
};

pub(crate) mod attr;
pub(crate) mod builder;
mod r#enum;
mod shared;
pub(crate) mod r#struct;

macro_rules! dbg {
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                #[cfg(feature = "debug")]
                std::fs::write(
                    &format!("log/{}.rs", {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::hash::DefaultHasher::new();
                        tmp.to_string().hash(&mut hasher);
                        hasher.finish()
                    }),
                    tmp.to_string().as_bytes(),
                )
                .unwrap();

                println!("cache???");

                tmp
            },
        }
    };
}

// macro_rules! dbg {
//     ($val:expr $(,)?) => {
//         // Use of `match` here is intentional because it affects the lifetimes
//         // of temporaries - https://stackoverflow.com/a/48732525/1063961
//         match $val {
//             tmp => {
//                 #[cfg(feature = "debug")]
//                 eprintln!(
//                     "[{}:{}:{}] {} = {}",
//                     std::file!(),
//                     std::line!(),
//                     std::column!(),
//                     std::stringify!($val),
//                     &tmp
//                 );
//                 tmp
//             },
//         }
//     };
// }

#[proc_macro_attribute]
pub fn mlua_gen(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;

    let mut attributes = Attributes::default();
    let attr_parser = syn::meta::parser(|meta| attributes.parse(&meta));
    parse_macro_input!(args with attr_parser);

    let code = match input.data {
        Data::Struct(ref ds) => {
            match (|| -> syn::Result<TokenStream2> {
                let field_get = attributes.get.fields_from_visibility(&ds.fields)?;
                let field_set = attributes.set.fields_from_visibility(&ds.fields)?;

                let builder = r#struct::builder(
                    name,
                    ds,
                    attributes
                        .r#impl
                        .iter()
                        .filter(|fun| !fun.is_self)
                        .collect(),
                    generics,
                );

                let user_data = r#struct::user_data(
                    name,
                    generics,
                    &ds.fields,
                    // GET TYPE AND IDENT
                    field_get,
                    field_set,
                    attributes.custom_fields,
                    attributes.r#impl,
                    attributes.custom_impls,
                );

                Ok(quote!(#builder #user_data))
            })() {
                Ok(e) => dbg!(e),
                // Ok(e) => e,
                Err(synerr) => return synerr.into_compile_error().into(),
            }
        },
        Data::Enum(ref de) => {
            let builder = r#enum::builder(
                name,
                de,
                attributes
                    .r#impl
                    .iter()
                    .filter(|fun| !fun.is_self)
                    .collect(),
                generics,
            );
            let user_data = r#enum::user_data(
                name,
                generics,
                de.variants.iter(),
                attributes.custom_fields,
                attributes.custom_impls,
            );
            quote!(#builder #user_data)
        },
        Data::Union(_) => panic!("Must annotate struct or enum"),
    };

    quote! {
        #input

        #code
    }
    .into()
}
