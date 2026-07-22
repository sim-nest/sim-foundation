use super::markers::{parse_codec, parse_macro, parse_number_domain, parse_site};
use super::types::parse_type;
use super::{
    ParsedCase, ParsedClass, ParsedField, ParsedFunction, ParsedFunctionKind, ParsedModule,
};
use crate::attrs::{has_attr, lit_str_attr_from_attrs, string_attr_from_attrs, strip_attrs};
use quote::format_ident;
use syn::{Attribute, Fields, Item, ItemFn, ItemStruct, ReturnType};

impl ParsedModule {
    pub(crate) fn parse(items: &mut [Item]) -> syn::Result<Self> {
        let mut classes = Vec::new();
        let mut functions = Vec::new();
        let mut macros = Vec::new();
        let mut codecs = Vec::new();
        let mut number_domains = Vec::new();
        let mut sites = Vec::new();
        let mut cleaned_items = Vec::new();

        for item in items.iter().cloned() {
            match item {
                Item::Struct(mut item_struct) if has_attr(&item_struct.attrs, "sim_class") => {
                    classes.push(parse_class(&mut item_struct)?);
                    cleaned_items.push(Item::Struct(item_struct));
                }
                Item::Fn(mut item_fn)
                    if has_attr(&item_fn.attrs, "sim_constructor")
                        || has_attr(&item_fn.attrs, "sim_fn") =>
                {
                    functions.push(parse_function(&mut item_fn, &classes)?);
                    cleaned_items.push(Item::Fn(item_fn));
                }
                Item::Fn(mut item_fn) if has_attr(&item_fn.attrs, "sim_macro") => {
                    macros.push(parse_macro(&mut item_fn)?);
                }
                Item::Fn(mut item_fn) if has_attr(&item_fn.attrs, "sim_codec") => {
                    codecs.push(parse_codec(&mut item_fn)?);
                }
                Item::Fn(mut item_fn) if has_attr(&item_fn.attrs, "sim_number_domain") => {
                    number_domains.push(parse_number_domain(&mut item_fn)?);
                }
                Item::Fn(mut item_fn) if has_attr(&item_fn.attrs, "sim_site") => {
                    sites.push(parse_site(&mut item_fn)?);
                }
                other => cleaned_items.push(other),
            }
        }

        Ok(Self {
            classes,
            functions,
            macros,
            codecs,
            number_domains,
            sites,
            cleaned_items,
        })
    }
}

fn parse_class(item: &mut ItemStruct) -> syn::Result<ParsedClass> {
    let lisp_name = string_attr_from_attrs(&item.attrs, "sim_class", "name")?
        .unwrap_or_else(|| item.ident.to_string());
    let shape_literal = lit_str_attr_from_attrs(&item.attrs, "shape", "")?;
    strip_attrs(&mut item.attrs, &["sim_class", "shape"]);
    let Fields::Named(fields) = &item.fields else {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "#[sim_class] requires named fields",
        ));
    };
    let mut parsed_fields = Vec::new();
    for field in &fields.named {
        let ident = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;
        parsed_fields.push(ParsedField {
            ident,
            ty: parse_type(&field.ty)?,
        });
    }
    Ok(ParsedClass {
        rust_ident: item.ident.clone(),
        lisp_name,
        wrapper_ident: format_ident!("__Lisp{}Value", item.ident),
        shape_literal,
        fields: parsed_fields,
    })
}

fn parse_function(item: &mut ItemFn, classes: &[ParsedClass]) -> syn::Result<ParsedFunction> {
    let kind = if has_attr(&item.attrs, "sim_constructor") {
        let class_name = string_attr_from_attrs(&item.attrs, "sim_constructor", "class")?
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    &item.sig.ident,
                    "#[sim_constructor] requires class = \"...\"",
                )
            })?;
        let class_ident = classes
            .iter()
            .find(|class| class.lisp_name == class_name || class.rust_ident == class_name.as_str())
            .map(|class| class.rust_ident.clone())
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    &item.sig.ident,
                    format!("unknown constructor class {class_name}"),
                )
            })?;
        ParsedFunctionKind::Constructor { class_ident }
    } else {
        ParsedFunctionKind::Function {
            lisp_name: string_attr_from_attrs(&item.attrs, "sim_fn", "name")?
                .unwrap_or_else(|| item.sig.ident.to_string()),
        }
    };

    let cases = case_attrs(&item.attrs)?;
    if cases.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.sig.ident,
            "expected at least one #[case(args = \"...\")]",
        ));
    }

    let mut inputs = Vec::new();
    for arg in &item.sig.inputs {
        let syn::FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new_spanned(arg, "methods are not supported"));
        };
        let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "unsupported argument pattern",
            ));
        };
        inputs.push((pat_ident.ident.clone(), parse_type(&arg.ty)?));
    }

    let output = match &item.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => Some(parse_type(ty)?),
    };

    strip_attrs(
        &mut item.attrs,
        &["sim_constructor", "sim_fn", "case", "shape"],
    );

    Ok(ParsedFunction {
        rust_ident: item.sig.ident.clone(),
        kind,
        cases,
        inputs,
        output,
    })
}

fn case_attrs(attrs: &[Attribute]) -> syn::Result<Vec<ParsedCase>> {
    let mut cases = Vec::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("case")) {
        let mut args = None;
        let mut result = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("args") {
                let value: syn::LitStr = meta.value()?.parse()?;
                args = Some(value);
                Ok(())
            } else if meta.path.is_ident("result") {
                let value: syn::LitStr = meta.value()?.parse()?;
                result = Some(value);
                Ok(())
            } else {
                Err(meta.error("unsupported #[case(...)] entry"))
            }
        })?;
        let args = args.ok_or_else(|| syn::Error::new_spanned(attr, "missing case args"))?;
        cases.push(ParsedCase { args, result });
    }
    Ok(cases)
}
