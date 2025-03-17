use {
    crate::attr::MethodOrFunction,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    std::borrow::Borrow,
    syn::{Field, Fields, FieldsNamed, FieldsUnnamed, parse_str},
};

pub(crate) fn builder_for_functions(
    name: &TokenStream2,
    fns: Vec<&MethodOrFunction>,
) -> TokenStream2 {
    if fns.is_empty() {
        quote!(Ok(None))
    } else {
        let public_functions = fns
            .into_iter()
            .map(|fun| {
                let (args, tys): (Vec<_>, Vec<_>) = fun
                    .args
                    .iter()
                    .enumerate()
                    .map(|(idx, ty)| {
                        (
                            parse_str::<syn::Ident>(&format!("a{idx}"))
                                .expect("This should always be a valid ident"),
                            parse_str::<syn::Type>(ty).expect("This should always be a valid type"),
                        )
                    })
                    .unzip();
                let fn_name = parse_str::<syn::Ident>(&fun.name)
                    .expect("This should always be a valid ident");
                quote! {
                    table.set(stringify!(#fn_name),
                        lua.create_function(|this, (#(#args),*): (#(#tys),*)| {
                            Ok(#name::#fn_name(#(#args),*))
                        })?
                    )?;
                }
            })
            .collect::<Vec<_>>();

        quote!(
            Ok({
                let table = lua.create_table()?;
                #(#public_functions)*
                Some(table)
            })
        )
    }
}

pub(crate) fn builder_for_fields(
    name: &TokenStream2,
    fields: &Fields,
    is_function_wrap: bool,
) -> TokenStream2 {
    match fields {
        Fields::Unit => quote! { Ok::<_, ::mlua::Error>(#name) },
        Fields::Unnamed(unnamed) => builder_for_unnamed(name, unnamed, is_function_wrap),
        Fields::Named(named) => builder_for_named(name, named, is_function_wrap),
    }
}

fn builder_for_unnamed(
    name: &TokenStream2,
    fields: &FieldsUnnamed,
    is_function_wrap: bool,
) -> TokenStream2 {
    let (access, tys) = generate_tuple_access(fields.unnamed.iter());
    let (first_arg, function_creation) = if is_function_wrap {
        (quote!(_: ::mlua::Table), quote!(::mlua::Function::wrap))
    } else {
        (quote!(_), quote!(lua.create_function))
    };

    quote! {
        #function_creation(|#first_arg, args: #tys| {
            Ok(#name (#access))
        })
    }
}

fn builder_for_named(
    name: &TokenStream2,
    fields: &FieldsNamed,
    is_function_wrap: bool,
) -> TokenStream2 {
    let names = fields.named.iter().map(|x| &x.ident);
    let (first_arg, function_creation) = if is_function_wrap {
        (quote!(_: ::mlua::Table), quote!(::mlua::Function::wrap))
    } else {
        (quote!(_), quote!(lua.create_function))
    };

    quote! {
        #function_creation(|#first_arg, data: ::mlua::Table| {
            Ok(#name {
                #( #names: data.get(stringify!(#names))?, )*
            })
        })
    }
}

/// Depending on if a tuple has 0, 1 or more elemnts, the syntax of a tuple is not the same.
///
/// 0 => Nothing
/// 1 => `(type)` is interepreted as `type` and therefore `.0` doesn't work
/// _ => Classic
///
/// The goal of this function is to generate accessors to the value `args.idx` and the types
/// associated to the accessed values `(my_type1, my_type2, ...)`
pub fn generate_tuple_access<'l, I: Iterator<Item = &'l Field>>(
    mut fields_unnamed: I,
) -> (TokenStream2, TokenStream2) {
    match (fields_unnamed.next(), fields_unnamed.next()) {
        (None, _) => (quote!(), quote! {()}),
        // `(my_type)` <=> `my_type` and therefore `.0` won't work
        (Some(field_unnamed), None) => {
            let ty = &field_unnamed.ty;

            (quote!(args), quote!((#ty)))
        },
        (Some(first), Some(second)) => {
            fn process_values<B: Borrow<Field>>(
                (idx, arg): (usize, &B),
            ) -> (TokenStream2, syn::Type) {
                let index = syn::Index::from(idx);
                (quote!(args.#index), arg.borrow().ty.clone())
            }

            let (mut args_ident, mut tys): (Vec<_>, Vec<_>) = [first, second]
                .iter()
                .enumerate()
                .map(process_values)
                .unzip();
            let (remaining_args_ident, remaining_tys): (Vec<_>, Vec<_>) = fields_unnamed
                .enumerate()
                .map(|(idx, value)| (idx + 2, value))
                .map(process_values)
                .unzip();

            args_ident.extend(remaining_args_ident);
            tys.extend(remaining_tys);

            (quote!(#(#args_ident),*), quote!((#(#tys),*)))
        },
    }
}
