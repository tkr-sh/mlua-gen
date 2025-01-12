use std::collections::VecDeque;

use quote::ToTokens;
use syn::{
    meta::ParseNestedMeta, spanned::Spanned, ExprArray, Fields, Ident, Token, UnOp, Visibility,
};

#[derive(Default, Debug)]
pub(crate) struct Attributes {
    pub(crate) get: FieldsVisibility,
    pub(crate) set: FieldsVisibility,
    pub(crate) r#impl: Vec<MethodOrFunction>,
    pub(crate) custom_fields: Option<Ident>,
    pub(crate) custom_impls: Option<Ident>,
}

#[derive(Debug)]
pub(crate) struct MethodOrFunction {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
    pub(crate) is_mut: bool,
    pub(crate) is_method: bool,
}

impl Attributes {
    pub fn parse(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
        if let Some(ident) = meta.path.get_ident() {
            match ident.to_string().as_str() {
                "get" => {
                    self.get = FieldsVisibility::parse(meta)?;
                    Ok(())
                },
                "set" => {
                    self.set = FieldsVisibility::parse(meta)?;
                    Ok(())
                },
                "impl" => {
                    let arr: ExprArray = meta.value()?.parse()?;

                    let mut vec_elements = vec![];
                    for elem in arr.elems {
                        if let syn::Expr::Call(fn_call) = elem {
                            if let syn::Expr::Path(ident) = *fn_call.func {
                                let mut args = fn_call
                                    .args
                                    .into_iter()
                                    .filter_map(|exp| match exp {
                                        syn::Expr::Path(ident) => Some(exprpath_to_string(ident)),
                                        syn::Expr::Reference(ident) => {
                                            Some(ident.into_token_stream().to_string())
                                        },
                                        _ => None,
                                    })
                                    .collect::<VecDeque<_>>();

                                // The first arg should be `self` | `&self` | `&mut self` | `mut
                                // self`
                                let first_arg = args.pop_front();
                                let first_arg = first_arg.as_deref();

                                vec_elements.push(MethodOrFunction {
                                    name: exprpath_to_string(ident),
                                    is_mut: matches!(
                                        &first_arg,
                                        Some("& mut self") | Some("mut self")
                                    ),
                                    is_method: matches!(
                                        &first_arg,
                                        Some("& mut self") | Some("& self")
                                    ),
                                    args: args.into(),
                                });
                            }
                        } else {
                            return Err(meta.error("Expected an identifier"));
                        }
                    }

                    self.r#impl = vec_elements;

                    Ok(())
                },
                "custom_fields" => {
                    self.custom_fields = Some(meta.value()?.parse()?);
                    Ok(())
                },
                "custom_impls" => {
                    self.custom_impls = Some(meta.value()?.parse()?);
                    Ok(())
                },
                _ => Err(meta.error(format!("Unexpected attribute name: {ident}"))),
            }
        } else {
            Err(meta.error("Expected an ident."))
        }
    }
}

#[derive(Default, Debug)]
pub(crate) enum FieldsVisibility {
    None,
    Pub,
    PubCrate,
    #[default]
    PubSuper,
    All,
    Custom(Vec<String>),
}

impl From<&Visibility> for FieldsVisibility {
    fn from(value: &Visibility) -> Self {
        match value {
            Visibility::Public(_) => FieldsVisibility::Pub,
            Visibility::Restricted(paren) => {
                match paren.paren_token.span.join().source_text().as_deref() {
                    Some("(crate)") => FieldsVisibility::PubCrate,
                    Some("(super)") => FieldsVisibility::PubSuper,
                    _ => panic!("Unexpected visibility"),
                }
            },
            Visibility::Inherited => FieldsVisibility::None,
        }
    }
}

impl FieldsVisibility {
    fn parse(meta: ParseNestedMeta) -> syn::Result<Self> {
        // `=` parsing
        meta.value()?;

        // Check for `*`
        if meta.input.peek(Token![*]) {
            return match meta.input.parse::<UnOp>().expect("Peeked") {
                UnOp::Deref(_) => Ok(FieldsVisibility::All),
                _ => Err(meta.error("Invalida token.")),
            };
        }

        // Check for `pub`
        if meta.input.peek(Token![pub]) {
            if let Ok(visibility) = meta.input.parse::<Visibility>() {
                return Ok((&visibility).into());
            }
        }

        // If it wasn't any previous, then it should be an array
        {
            let arr: ExprArray = meta.input.parse()?;

            let mut vec_elements = vec![];
            for elem in arr.elems {
                if let syn::Expr::Path(ident) = elem {
                    vec_elements.push(exprpath_to_string(ident));
                } else {
                    return Err(meta.error("Expected an identifier"));
                }
            }

            Ok(FieldsVisibility::Custom(vec_elements))
        }
    }

    pub(crate) fn fields_from_visibility(&self, fields: &Fields) -> syn::Result<Vec<String>> {
        match fields {
            Fields::Named(fields_named) => Ok(fields_named
                .named
                .iter()
                .filter(|field| match self {
                    FieldsVisibility::Pub => matches!((&field.vis).into(), FieldsVisibility::Pub),
                    FieldsVisibility::PubCrate => matches!(
                        (&field.vis).into(),
                        FieldsVisibility::Pub | FieldsVisibility::PubCrate
                    ),
                    FieldsVisibility::PubSuper => matches!(
                        (&field.vis).into(),
                        FieldsVisibility::Pub
                            | FieldsVisibility::PubCrate
                            | FieldsVisibility::PubSuper
                    ),
                    FieldsVisibility::Custom(v) => {
                        if let Some(ident) = field
                            .ident
                            .as_ref()
                            .expect("Is named => has ident")
                            .span()
                            .source_text()
                        {
                            v.contains(&ident)
                        } else {
                            false
                        }
                    },
                    FieldsVisibility::All => true,
                    FieldsVisibility::None => false,
                })
                .filter_map(|field| {
                    field
                        .ident
                        .as_ref()
                        .expect("Is named => has ident")
                        .span()
                        .source_text()
                })
                .collect()),
            Fields::Unnamed(_) => panic!("Shouldn't be a unnamed struct"),
            Fields::Unit => panic!("Shouldn't be a unit struct"),
        }
    }
}

fn exprpath_to_string(exprpath: syn::ExprPath) -> String {
    exprpath
        .path
        .span()
        .source_text()
        .expect("Should be a valid ident")
}
