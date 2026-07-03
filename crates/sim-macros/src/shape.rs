//! Compiles a shape s-expression literal into shape-building tokens.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::LitStr;

pub(crate) fn parse_shape_literal(input: &LitStr) -> syn::Result<TokenStream2> {
    let sexp =
        parse_sexp(&input.value()).map_err(|message| syn::Error::new(input.span(), message))?;
    shape_tokens(&sexp)
}

fn parse_shape_literal_for_class(input: &LitStr, class_symbol: &str) -> syn::Result<TokenStream2> {
    let sexp =
        parse_sexp(&input.value()).map_err(|message| syn::Error::new(input.span(), message))?;
    shape_tokens_for_class(&sexp, class_symbol)
}

pub(crate) fn class_shape_tokens(
    shape_literal: Option<&LitStr>,
    class_symbol: &str,
) -> syn::Result<TokenStream2> {
    shape_literal
        .map(|shape| parse_shape_literal_for_class(shape, class_symbol))
        .transpose()?
        .map_or_else(
            || {
                let class_symbol = LitStr::new(class_symbol, Span::call_site());
                Ok(quote!(::std::sync::Arc::new(::sim::shape::FieldShape::new(
                    ::sim::kernel::Symbol::new(#class_symbol),
                    vec![],
                ))))
            },
            Ok,
        )
}

#[derive(Clone)]
pub(crate) enum Sexp {
    Atom(String),
    List(Vec<Sexp>),
}

pub(crate) fn parse_sexp(input: &str) -> Result<Sexp, String> {
    let mut chars = input.chars().peekable();
    let sexp = parse_sexp_inner(&mut chars)?;
    skip_ws(&mut chars);
    if chars.peek().is_some() {
        Err("unexpected trailing input".to_owned())
    } else {
        Ok(sexp)
    }
}

fn parse_sexp_inner(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<Sexp, String> {
    skip_ws(chars);
    match chars.peek() {
        Some('(') => {
            chars.next();
            let mut items = Vec::new();
            loop {
                skip_ws(chars);
                match chars.peek() {
                    Some(')') => {
                        chars.next();
                        break;
                    }
                    Some(_) => items.push(parse_sexp_inner(chars)?),
                    None => return Err("unterminated list".to_owned()),
                }
            }
            Ok(Sexp::List(items))
        }
        Some(_) => {
            let mut atom = String::new();
            while let Some(ch) = chars.peek().copied() {
                if ch.is_whitespace() || ch == '(' || ch == ')' {
                    break;
                }
                atom.push(ch);
                chars.next();
            }
            Ok(Sexp::Atom(atom))
        }
        None => Err("unexpected end of input".to_owned()),
    }
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
}

fn shape_tokens(sexp: &Sexp) -> syn::Result<TokenStream2> {
    shape_tokens_with_class(sexp, None)
}

fn shape_tokens_for_class(sexp: &Sexp, class_symbol: &str) -> syn::Result<TokenStream2> {
    shape_tokens_with_class(sexp, Some(class_symbol))
}

fn shape_tokens_with_class(sexp: &Sexp, class_symbol: Option<&str>) -> syn::Result<TokenStream2> {
    Ok(match sexp {
        Sexp::Atom(atom) => match atom.as_str() {
            "Any" => quote!(::std::sync::Arc::new(::sim::shape::AnyShape)),
            "Number" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::Number
            ))),
            "String" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::String
            ))),
            "Bool" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::Bool
            ))),
            "Symbol" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::Symbol
            ))),
            "Map" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::Map
            ))),
            "List" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::List
            ))),
            "Nil" => quote!(::std::sync::Arc::new(::sim::shape::ExprKindShape::new(
                ::sim::shape::ExprKind::Nil
            ))),
            other => {
                let symbol = LitStr::new(other, Span::call_site());
                quote!(::std::sync::Arc::new(::sim::shape::ClassShape::new(::sim::kernel::Symbol::new(#symbol))))
            }
        },
        Sexp::List(items) => {
            if let Some(Sexp::Atom(head)) = items.first() {
                if head == "capture" && items.len() == 3 {
                    let name = match &items[1] {
                        Sexp::Atom(name) => LitStr::new(name, Span::call_site()),
                        _ => {
                            return Err(syn::Error::new(
                                Span::call_site(),
                                "capture shape requires a symbolic binding name",
                            ));
                        }
                    };
                    let inner = shape_tokens_with_class(&items[2], None)?;
                    return Ok(quote! {
                        ::std::sync::Arc::new(::sim::shape::CaptureShape::new(
                            ::sim::kernel::Symbol::new(#name),
                            #inner,
                        ))
                    });
                }
                if head == "fields" {
                    let specs = items
                        .iter()
                        .skip(1)
                        .map(field_spec_tokens)
                        .collect::<syn::Result<Vec<_>>>()?;
                    return Ok(match class_symbol {
                        Some(class_symbol) => {
                            let class_symbol = LitStr::new(class_symbol, Span::call_site());
                            quote! {
                                ::std::sync::Arc::new(::sim::shape::FieldShape::new(
                                    ::sim::kernel::Symbol::new(#class_symbol),
                                    vec![#(#specs),*],
                                ))
                            }
                        }
                        None => quote! {
                            ::std::sync::Arc::new(::sim::shape::FieldShape::anonymous(
                                vec![#(#specs),*],
                            ))
                        },
                    });
                }
            }
            let items = items
                .iter()
                .map(|item| shape_tokens_with_class(item, None))
                .collect::<syn::Result<Vec<_>>>()?;
            quote!(::std::sync::Arc::new(::sim::shape::ListShape::new(
                vec![#(#items),*]
            )))
        }
    })
}

fn field_spec_tokens(sexp: &Sexp) -> syn::Result<TokenStream2> {
    let Sexp::List(items) = sexp else {
        return Err(syn::Error::new(
            Span::call_site(),
            "field spec must be a list like (:name Shape)",
        ));
    };
    let [Sexp::Atom(field), shape] = items.as_slice() else {
        return Err(syn::Error::new(
            Span::call_site(),
            "field spec must contain exactly a field name and shape",
        ));
    };
    let field = LitStr::new(field.trim_start_matches(':'), Span::call_site());
    let shape = shape_tokens_with_class(shape, None)?;
    Ok(quote!(::sim::shape::FieldSpec::required(
        ::sim::kernel::Symbol::new(#field),
        #shape,
    )))
}
