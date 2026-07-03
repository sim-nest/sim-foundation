//! Parses macro input into the intermediate Parsed* model.

use crate::attrs::{has_attr, lit_str_attr_from_attrs, string_attr_from_attrs, strip_attrs};
use quote::format_ident;
use syn::{Attribute, Fields, Ident, Item, ItemFn, ItemStruct, LitStr, ReturnType, Type};

#[derive(Clone)]
pub(crate) struct ParsedField {
    pub(crate) ident: Ident,
    pub(crate) ty: ParsedType,
}

#[derive(Clone)]
pub(crate) enum ParsedType {
    F64,
    Bool,
    String,
    Symbol,
    Expr,
    RefClass(Ident),
    OwnedClass(Ident),
}

pub(crate) struct ParsedClass {
    pub(crate) rust_ident: Ident,
    pub(crate) lisp_name: String,
    pub(crate) wrapper_ident: Ident,
    pub(crate) shape_literal: Option<LitStr>,
    pub(crate) fields: Vec<ParsedField>,
}

#[derive(Clone)]
pub(crate) struct ParsedCase {
    pub(crate) args: LitStr,
    pub(crate) result: Option<LitStr>,
}

pub(crate) enum ParsedFunctionKind {
    Constructor { class_ident: Ident },
    Function { lisp_name: String },
}

pub(crate) struct ParsedFunction {
    pub(crate) rust_ident: Ident,
    pub(crate) kind: ParsedFunctionKind,
    pub(crate) cases: Vec<ParsedCase>,
    pub(crate) inputs: Vec<(Ident, ParsedType)>,
    pub(crate) output: Option<ParsedType>,
}

pub(crate) struct ParsedCodec {
    pub(crate) symbol: String,
    pub(crate) decode_ident: Ident,
    pub(crate) encode_ident: Ident,
}

pub(crate) struct ParsedMacro {
    pub(crate) symbol: String,
    pub(crate) expand_ident: Ident,
}

pub(crate) struct ParsedNumberDomain {
    pub(crate) symbol: String,
    pub(crate) parse_ident: Ident,
    pub(crate) encode_ident: Ident,
    pub(crate) add_ident: Option<Ident>,
    pub(crate) sub_ident: Option<Ident>,
    pub(crate) mul_ident: Option<Ident>,
    pub(crate) div_ident: Option<Ident>,
    pub(crate) neg_ident: Option<Ident>,
    pub(crate) sum_ident: Option<Ident>,
    pub(crate) product_ident: Option<Ident>,
}

pub(crate) struct ParsedSite {
    pub(crate) symbol: String,
    pub(crate) realize_ident: Ident,
}

pub(crate) struct ParsedModule {
    pub(crate) classes: Vec<ParsedClass>,
    pub(crate) functions: Vec<ParsedFunction>,
    pub(crate) macros: Vec<ParsedMacro>,
    pub(crate) codecs: Vec<ParsedCodec>,
    pub(crate) number_domains: Vec<ParsedNumberDomain>,
    pub(crate) sites: Vec<ParsedSite>,
    pub(crate) cleaned_items: Vec<Item>,
}

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

impl ParsedFunction {
    pub(crate) fn class_id_args(&self) -> Vec<Ident> {
        unique_class_id_idents(&self.inputs)
    }
}

pub(crate) fn parse_class(item: &mut ItemStruct) -> syn::Result<ParsedClass> {
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

pub(crate) fn parse_function(
    item: &mut ItemFn,
    classes: &[ParsedClass],
) -> syn::Result<ParsedFunction> {
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

pub(crate) fn parse_codec(item: &mut ItemFn) -> syn::Result<ParsedCodec> {
    let matching_attrs = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("sim_codec"))
        .collect::<Vec<_>>();
    let attr = matching_attrs
        .first()
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "missing #[sim_codec]"))?;
    if matching_attrs.len() > 1 {
        return Err(syn::Error::new_spanned(
            attr,
            "duplicate #[sim_codec(...)] attribute",
        ));
    }

    let mut symbol = None;
    let mut decode = None;
    let mut encode = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("symbol") {
            symbol = Some(parse_unique_lit_str(&meta, symbol.as_ref(), "symbol")?);
            Ok(())
        } else if meta.path.is_ident("decode") {
            decode = Some(parse_unique_lit_str(&meta, decode.as_ref(), "decode")?);
            Ok(())
        } else if meta.path.is_ident("encode") {
            encode = Some(parse_unique_lit_str(&meta, encode.as_ref(), "encode")?);
            Ok(())
        } else {
            Err(meta.error("unsupported #[sim_codec(...)] entry"))
        }
    })?;

    let symbol = symbol
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "#[sim_codec] requires symbol"))?
        .value();
    let decode_ident = codec_ident_attr(&item.sig.ident, "decode", decode)?;
    let encode_ident = codec_ident_attr(&item.sig.ident, "encode", encode)?;
    strip_attrs(&mut item.attrs, &["sim_codec"]);
    Ok(ParsedCodec {
        symbol,
        decode_ident,
        encode_ident,
    })
}

pub(crate) fn parse_macro(item: &mut ItemFn) -> syn::Result<ParsedMacro> {
    let matching_attrs = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("sim_macro"))
        .collect::<Vec<_>>();
    let attr = matching_attrs
        .first()
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "missing #[sim_macro]"))?;
    if matching_attrs.len() > 1 {
        return Err(syn::Error::new_spanned(
            attr,
            "duplicate #[sim_macro(...)] attribute",
        ));
    }

    let mut symbol = None;
    let mut expand = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("symbol") {
            symbol = Some(parse_unique_lit_str(&meta, symbol.as_ref(), "symbol")?);
            Ok(())
        } else if meta.path.is_ident("expand") {
            expand = Some(parse_unique_lit_str(&meta, expand.as_ref(), "expand")?);
            Ok(())
        } else {
            Err(meta.error("unsupported #[sim_macro(...)] entry"))
        }
    })?;

    let symbol = symbol
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "#[sim_macro] requires symbol"))?
        .value();
    let expand_ident = marker_ident_attr(&item.sig.ident, "sim_macro", "expand", expand)?;
    strip_attrs(&mut item.attrs, &["sim_macro"]);
    Ok(ParsedMacro {
        symbol,
        expand_ident,
    })
}

pub(crate) fn parse_number_domain(item: &mut ItemFn) -> syn::Result<ParsedNumberDomain> {
    let matching_attrs = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("sim_number_domain"))
        .collect::<Vec<_>>();
    let attr = matching_attrs
        .first()
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "missing #[sim_number_domain]"))?;
    if matching_attrs.len() > 1 {
        return Err(syn::Error::new_spanned(
            attr,
            "duplicate #[sim_number_domain(...)] attribute",
        ));
    }

    let mut symbol = None;
    let mut parse = None;
    let mut encode = None;
    let mut add = None;
    let mut sub = None;
    let mut mul = None;
    let mut div = None;
    let mut neg = None;
    let mut sum = None;
    let mut product = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("symbol") {
            symbol = Some(parse_unique_lit_str(&meta, symbol.as_ref(), "symbol")?);
            Ok(())
        } else if meta.path.is_ident("parse") {
            parse = Some(parse_unique_lit_str(&meta, parse.as_ref(), "parse")?);
            Ok(())
        } else if meta.path.is_ident("encode") {
            encode = Some(parse_unique_lit_str(&meta, encode.as_ref(), "encode")?);
            Ok(())
        } else if meta.path.is_ident("add") {
            add = Some(parse_unique_lit_str(&meta, add.as_ref(), "add")?);
            Ok(())
        } else if meta.path.is_ident("sub") {
            sub = Some(parse_unique_lit_str(&meta, sub.as_ref(), "sub")?);
            Ok(())
        } else if meta.path.is_ident("mul") {
            mul = Some(parse_unique_lit_str(&meta, mul.as_ref(), "mul")?);
            Ok(())
        } else if meta.path.is_ident("div") {
            div = Some(parse_unique_lit_str(&meta, div.as_ref(), "div")?);
            Ok(())
        } else if meta.path.is_ident("neg") {
            neg = Some(parse_unique_lit_str(&meta, neg.as_ref(), "neg")?);
            Ok(())
        } else if meta.path.is_ident("sum") {
            sum = Some(parse_unique_lit_str(&meta, sum.as_ref(), "sum")?);
            Ok(())
        } else if meta.path.is_ident("product") {
            product = Some(parse_unique_lit_str(&meta, product.as_ref(), "product")?);
            Ok(())
        } else {
            Err(meta.error("unsupported #[sim_number_domain(...)] entry"))
        }
    })?;

    let symbol = symbol
        .ok_or_else(|| {
            syn::Error::new_spanned(&item.sig.ident, "#[sim_number_domain] requires symbol")
        })?
        .value();
    let parse_ident = marker_ident_attr(&item.sig.ident, "sim_number_domain", "parse", parse)?;
    let encode_ident = marker_ident_attr(&item.sig.ident, "sim_number_domain", "encode", encode)?;
    strip_attrs(&mut item.attrs, &["sim_number_domain"]);
    Ok(ParsedNumberDomain {
        symbol,
        parse_ident,
        encode_ident,
        add_ident: optional_marker_ident_attr("sim_number_domain", "add", add)?,
        sub_ident: optional_marker_ident_attr("sim_number_domain", "sub", sub)?,
        mul_ident: optional_marker_ident_attr("sim_number_domain", "mul", mul)?,
        div_ident: optional_marker_ident_attr("sim_number_domain", "div", div)?,
        neg_ident: optional_marker_ident_attr("sim_number_domain", "neg", neg)?,
        sum_ident: optional_marker_ident_attr("sim_number_domain", "sum", sum)?,
        product_ident: optional_marker_ident_attr("sim_number_domain", "product", product)?,
    })
}

pub(crate) fn parse_site(item: &mut ItemFn) -> syn::Result<ParsedSite> {
    let matching_attrs = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("sim_site"))
        .collect::<Vec<_>>();
    let attr = matching_attrs
        .first()
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "missing #[sim_site]"))?;
    if matching_attrs.len() > 1 {
        return Err(syn::Error::new_spanned(
            attr,
            "duplicate #[sim_site(...)] attribute",
        ));
    }

    let mut symbol = None;
    let mut realize = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("symbol") {
            symbol = Some(parse_unique_lit_str(&meta, symbol.as_ref(), "symbol")?);
            Ok(())
        } else if meta.path.is_ident("realize") {
            realize = Some(parse_unique_lit_str(&meta, realize.as_ref(), "realize")?);
            Ok(())
        } else {
            Err(meta.error("unsupported #[sim_site(...)] entry"))
        }
    })?;

    let symbol = symbol
        .ok_or_else(|| syn::Error::new_spanned(&item.sig.ident, "#[sim_site] requires symbol"))?
        .value();
    let realize_ident = marker_ident_attr(&item.sig.ident, "sim_site", "realize", realize)?;
    strip_attrs(&mut item.attrs, &["sim_site"]);
    Ok(ParsedSite {
        symbol,
        realize_ident,
    })
}

pub(crate) fn parse_type(ty: &Type) -> syn::Result<ParsedType> {
    match ty {
        Type::Path(path) if path.path.is_ident("f64") => Ok(ParsedType::F64),
        Type::Path(path) if path.path.is_ident("bool") => Ok(ParsedType::Bool),
        Type::Path(path) => {
            let ident = path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.clone())
                .ok_or_else(|| syn::Error::new_spanned(path, "expected type path"))?;
            match ident.to_string().as_str() {
                "String" => Ok(ParsedType::String),
                "Symbol" => Ok(ParsedType::Symbol),
                "Expr" => Ok(ParsedType::Expr),
                _ => Ok(ParsedType::OwnedClass(ident)),
            }
        }
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Path(path) => {
                let ident = path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.clone())
                    .ok_or_else(|| syn::Error::new_spanned(path, "expected reference type path"))?;
                match ident.to_string().as_str() {
                    "str" => Err(syn::Error::new_spanned(
                        path,
                        "borrowed builtin types are not supported here",
                    )),
                    "String" | "Symbol" | "Expr" | "f64" | "bool" => Err(syn::Error::new_spanned(
                        path,
                        "borrowed builtin types are not supported here",
                    )),
                    _ => Ok(ParsedType::RefClass(ident)),
                }
            }
            other => Err(syn::Error::new_spanned(other, "unsupported reference type")),
        },
        other => Err(syn::Error::new_spanned(
            other,
            "unsupported type in proc macro lib",
        )),
    }
}

fn parse_unique_lit_str(
    meta: &syn::meta::ParseNestedMeta<'_>,
    existing: Option<&LitStr>,
    name: &str,
) -> syn::Result<LitStr> {
    if existing.is_some() {
        return Err(meta.error(format!("duplicate {name} entry in marker attribute")));
    }
    meta.value()?.parse()
}

fn codec_ident_attr(
    item_ident: &Ident,
    nested_name: &str,
    value: Option<LitStr>,
) -> syn::Result<Ident> {
    let value = value.ok_or_else(|| {
        syn::Error::new_spanned(item_ident, format!("#[sim_codec] requires {nested_name}"))
    })?;
    let raw = value.value();
    syn::parse_str::<Ident>(&raw).map_err(|_| {
        syn::Error::new_spanned(
            value,
            format!("#[sim_codec] {nested_name} must name a Rust function"),
        )
    })
}

fn marker_ident_attr(
    item_ident: &Ident,
    marker: &str,
    nested_name: &str,
    value: Option<LitStr>,
) -> syn::Result<Ident> {
    let value = value.ok_or_else(|| {
        syn::Error::new_spanned(item_ident, format!("#[{marker}] requires {nested_name}"))
    })?;
    parse_marker_ident(marker, nested_name, value)
}

fn optional_marker_ident_attr(
    marker: &str,
    nested_name: &str,
    value: Option<LitStr>,
) -> syn::Result<Option<Ident>> {
    value
        .map(|value| parse_marker_ident(marker, nested_name, value))
        .transpose()
}

fn parse_marker_ident(marker: &str, nested_name: &str, value: LitStr) -> syn::Result<Ident> {
    let raw = value.value();
    syn::parse_str::<Ident>(&raw).map_err(|_| {
        syn::Error::new_spanned(
            value,
            format!("#[{marker}] {nested_name} must name a Rust function"),
        )
    })
}

fn case_attrs(attrs: &[Attribute]) -> syn::Result<Vec<ParsedCase>> {
    let mut cases = Vec::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("case")) {
        let mut args = None;
        let mut result = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("args") {
                let value: LitStr = meta.value()?.parse()?;
                args = Some(value);
                Ok(())
            } else if meta.path.is_ident("result") {
                let value: LitStr = meta.value()?.parse()?;
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

pub(crate) fn unique_class_id_idents(inputs: &[(Ident, ParsedType)]) -> Vec<Ident> {
    let mut out = Vec::new();
    for (_, ty) in inputs {
        if let ParsedType::RefClass(class) | ParsedType::OwnedClass(class) = ty {
            let ident = format_ident!("__lisp_class_id_{}", class.to_string().to_lowercase());
            if !out.iter().any(|existing| existing == &ident) {
                out.push(ident);
            }
        }
    }
    out
}
