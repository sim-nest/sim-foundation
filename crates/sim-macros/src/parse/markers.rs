use super::{ParsedCodec, ParsedMacro, ParsedNumberDomain, ParsedSite};
use crate::attrs::strip_attrs;
use syn::{Ident, ItemFn, LitStr};

pub(super) fn parse_codec(item: &mut ItemFn) -> syn::Result<ParsedCodec> {
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

pub(super) fn parse_macro(item: &mut ItemFn) -> syn::Result<ParsedMacro> {
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

pub(super) fn parse_number_domain(item: &mut ItemFn) -> syn::Result<ParsedNumberDomain> {
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

pub(super) fn parse_site(item: &mut ItemFn) -> syn::Result<ParsedSite> {
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
