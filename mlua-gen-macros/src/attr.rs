use {
    quote::ToTokens,
    std::collections::VecDeque,
    syn::{
        ExprArray,
        Fields,
        Ident,
        Token,
        UnOp,
        Visibility,
        meta::ParseNestedMeta,
        spanned::Spanned,
    },
};

#[derive(Default, Debug)]
pub(crate) struct Attributes {
    pub(crate) get:           FieldsVisibility,
    pub(crate) set:           FieldsVisibility,
    pub(crate) r#impl:        Vec<MethodOrFunction>,
    pub(crate) custom_fields: Option<Ident>,
    pub(crate) custom_impls:  Option<Ident>,
}

#[derive(Debug)]
pub(crate) struct MethodOrFunction {
    pub(crate) name:    String,
    pub(crate) args:    Vec<String>,
    pub(crate) is_mut:  bool,
    // Unused for now
    // pub(crate) is_ref:  bool,
    pub(crate) is_self: bool,
}

impl Attributes {
    pub fn parse(&mut self, meta: &ParseNestedMeta) -> syn::Result<()> {
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
                                    .filter_map(|exp| {
                                        match exp {
                                            syn::Expr::Path(ident) => {
                                                Some(exprpath_to_string(&ident))
                                            },
                                            syn::Expr::Reference(ident) => {
                                                Some(Ok(ident.into_token_stream().to_string()))
                                            },
                                            _ => None,
                                        }
                                    })
                                    .collect::<syn::Result<VecDeque<_>>>()?;

                                let first_arg = args.front();
                                let first_arg = first_arg.map(std::string::String::as_str);
                                let is_self = matches!(
                                    &first_arg,
                                    Some("& mut self" | "mut self" | "& self" | "self")
                                );

                                vec_elements.push(MethodOrFunction {
                                    name: exprpath_to_string(&ident)?,
                                    is_mut: matches!(&first_arg, Some("& mut self" | "mut self")),
                                    // Unused for now
                                    // is_ref: matches!(
                                    //     &first_arg,
                                    //     Some("& mut self") | Some("& self")
                                    // ),
                                    is_self,
                                    args: {
                                        if is_self {
                                            args.pop_front();
                                        }
                                        args.into()
                                    },
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

impl TryFrom<&Visibility> for FieldsVisibility {
    type Error = syn::Error;

    fn try_from(value: &Visibility) -> Result<Self, Self::Error> {
        Ok(match value {
            Visibility::Public(_) => FieldsVisibility::Pub,
            Visibility::Restricted(paren) => {
                match paren.paren_token.span.join().source_text().as_deref() {
                    Some("(crate)") => FieldsVisibility::PubCrate,
                    Some("(super)") => FieldsVisibility::PubSuper,
                    _ => return Err(syn::Error::new(value.span(), "Unexpected visibility")),
                }
            },
            Visibility::Inherited => FieldsVisibility::None,
        })
    }
}

impl FieldsVisibility {
    fn parse(meta: &ParseNestedMeta) -> syn::Result<Self> {
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
                return (&visibility).try_into();
            }
        }

        // If it wasn't any previous, then it should be an array
        {
            let arr: ExprArray = meta.input.parse()?;

            let mut vec_elements = vec![];
            for elem in arr.elems {
                if let syn::Expr::Path(ident) = elem {
                    vec_elements.push(exprpath_to_string(&ident)?);
                } else {
                    return Err(meta.error("Expected an identifier"));
                }
            }

            Ok(FieldsVisibility::Custom(vec_elements))
        }
    }

    pub(crate) fn fields_from_visibility(&self, fields: &Fields) -> syn::Result<Vec<String>> {
        match fields {
            Fields::Named(fields_named) => {
                Ok(fields_named
                    .named
                    .iter()
                    .filter(|field| {
                        match self {
                            FieldsVisibility::Pub => {
                                matches!(
                                    (&field.vis).try_into().expect("Is Pub"),
                                    FieldsVisibility::Pub
                                )
                            },
                            FieldsVisibility::PubCrate => {
                                matches!(
                                    (&field.vis).try_into().expect("Is PubCrate"),
                                    FieldsVisibility::Pub | FieldsVisibility::PubCrate
                                )
                            },
                            FieldsVisibility::PubSuper => {
                                matches!(
                                    (&field.vis).try_into().expect("Is PubSuper"),
                                    FieldsVisibility::Pub |
                                        FieldsVisibility::PubCrate |
                                        FieldsVisibility::PubSuper
                                )
                            },
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
                        }
                    })
                    .filter_map(|field| {
                        field
                            .ident
                            .as_ref()
                            .expect("Is named => has ident")
                            .span()
                            .source_text()
                    })
                    .collect())
            },
            Fields::Unnamed(fields_unnamed) => {
                Ok(fields_unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .filter(|(idx, field)| {
                        match self {
                            FieldsVisibility::Pub => {
                                matches!(
                                    (&field.vis).try_into().expect("Is Pub"),
                                    FieldsVisibility::Pub
                                )
                            },
                            FieldsVisibility::PubCrate => {
                                matches!(
                                    (&field.vis).try_into().expect("Is PubCrate"),
                                    FieldsVisibility::Pub | FieldsVisibility::PubCrate
                                )
                            },
                            FieldsVisibility::PubSuper => {
                                matches!(
                                    (&field.vis).try_into().expect("Is PubSuper"),
                                    FieldsVisibility::Pub |
                                        FieldsVisibility::PubCrate |
                                        FieldsVisibility::PubSuper
                                )
                            },
                            FieldsVisibility::Custom(v) => {
                                let field = idx.to_string();
                                v.contains(&field)
                            },
                            FieldsVisibility::All => true,
                            FieldsVisibility::None => false,
                        }
                    })
                    .map(|(idx, _)| idx.to_string())
                    .collect())
            },
            Fields::Unit => Ok(Vec::new()),
        }
    }
}

fn exprpath_to_string(exprpath: &syn::ExprPath) -> syn::Result<String> {
    exprpath.path.span().source_text().ok_or_else(|| {
        syn::Error::new(
            exprpath.span(),
            format!("Expected {exprpath:?} to have a valid `source_text`"),
        )
    })
}
