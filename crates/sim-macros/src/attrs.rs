//! Attribute parsing helpers for the SIM derive and function macros.

use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::{Attribute, Expr, ExprLit, Lit, LitStr, Token, punctuated::Punctuated};

pub(crate) fn string_attr(
    attrs: &Punctuated<syn::MetaNameValue, Token![,]>,
    name: &str,
) -> Result<String, TokenStream> {
    string_attr_value(attrs, name).map_err(|err| err.to_compile_error().into())
}

pub(crate) fn bool_attr(
    attrs: &Punctuated<syn::MetaNameValue, Token![,]>,
    name: &str,
) -> Result<bool, TokenStream> {
    bool_attr_value(attrs, name).map_err(|err| err.to_compile_error().into())
}

pub(crate) fn string_attr_value(
    attrs: &Punctuated<syn::MetaNameValue, Token![,]>,
    name: &str,
) -> syn::Result<String> {
    let matches = attrs
        .iter()
        .filter(|attr| attr.path.is_ident(name))
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(syn::Error::new(
            Span::call_site(),
            format!("duplicate #[sim_lib({name} = ...)] entry"),
        ));
    }
    matches
        .first()
        .and_then(|attr| match &attr.value {
            Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) => Some(value.value()),
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                format!("expected #[sim_lib({name} = \"...\")]"),
            )
        })
}

pub(crate) fn bool_attr_value(
    attrs: &Punctuated<syn::MetaNameValue, Token![,]>,
    name: &str,
) -> syn::Result<bool> {
    let matches = attrs
        .iter()
        .filter(|attr| attr.path.is_ident(name))
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(syn::Error::new(
            Span::call_site(),
            format!("duplicate #[sim_lib({name} = ...)] entry"),
        ));
    }
    match matches.first().map(|attr| &attr.value) {
        None => Ok(false),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Bool(value),
            ..
        })) => Ok(value.value),
        Some(_) => Err(syn::Error::new(
            Span::call_site(),
            format!("expected #[sim_lib({name} = true|false)]"),
        )),
    }
}

pub(crate) fn string_attr_from_attrs(
    attrs: &[Attribute],
    attr_name: &str,
    nested_name: &str,
) -> syn::Result<Option<String>> {
    Ok(lit_str_attr_from_attrs(attrs, attr_name, nested_name)?.map(|value| value.value()))
}

pub(crate) fn lit_str_attr_from_attrs(
    attrs: &[Attribute],
    attr_name: &str,
    nested_name: &str,
) -> syn::Result<Option<LitStr>> {
    let matching_attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident(attr_name))
        .collect::<Vec<_>>();
    let Some(attr) = matching_attrs.first() else {
        return Ok(None);
    };
    if matching_attrs.len() > 1 {
        return Err(syn::Error::new_spanned(
            attr,
            format!("duplicate #[{attr_name}(...)] attribute"),
        ));
    }
    if nested_name.is_empty() {
        match &attr.meta {
            syn::Meta::List(list) => {
                let lit: LitStr = list.parse_args()?;
                Ok(Some(lit))
            }
            _ => Err(syn::Error::new_spanned(attr, "expected string attribute")),
        }
    } else {
        let mut found = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(nested_name) {
                if found.is_some() {
                    return Err(meta.error(format!(
                        "duplicate {nested_name} entry in #[{attr_name}(...)]"
                    )));
                }
                let value: LitStr = meta.value()?.parse()?;
                found = Some(value);
                Ok(())
            } else {
                Ok(())
            }
        })?;
        Ok(found)
    }
}

pub(crate) fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

pub(crate) fn strip_attrs(attrs: &mut Vec<Attribute>, names: &[&str]) {
    attrs.retain(|attr| !names.iter().any(|name| attr.path().is_ident(name)));
}

pub(crate) fn camel_case(input: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in input.chars() {
        if ch == '_' || ch == '-' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}
