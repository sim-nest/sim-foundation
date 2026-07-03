use crate::attrs::{camel_case, string_attr_from_attrs, string_attr_value};
use crate::parse::ParsedModule;
use crate::shape::{Sexp, parse_sexp};
use quote::format_ident;
use syn::{Item, Token, parse::Parser, parse_quote, punctuated::Punctuated};

#[test]
fn duplicate_lisp_lib_keys_are_rejected() {
    let parser = Punctuated::<syn::MetaNameValue, Token![,]>::parse_terminated;
    let attrs = parser
        .parse2(parse_quote! { id = "one", id = "two" })
        .unwrap();
    assert!(string_attr_value(&attrs, "id").is_err());
}

#[test]
fn duplicate_nested_attribute_keys_are_rejected() {
    let item: Item = parse_quote! {
        #[sim_class(name = "A", name = "B")]
        struct Point { x: f64 }
    };
    let Item::Struct(item) = item else {
        panic!("expected struct");
    };
    let err = string_attr_from_attrs(&item.attrs, "sim_class", "name").unwrap_err();
    assert!(err.to_string().contains("duplicate name entry"));
}

#[test]
fn tuple_struct_lisp_class_is_rejected() {
    let mut items = vec![parse_quote! {
        #[sim_class(name = "Point")]
        struct Point(f64, f64);
    }];
    let err = ParsedModule::parse(&mut items).err().unwrap();
    assert!(err.to_string().contains("named fields"));
}

#[test]
fn unknown_constructor_class_is_rejected() {
    let mut items = vec![
        parse_quote! {
            #[sim_class(name = "Point")]
            struct Point { x: f64, y: f64 }
        },
        parse_quote! {
            #[sim_constructor(class = "Missing")]
            #[case(args = "((capture x Number) (capture y Number))", result = "Missing")]
            fn make_missing(x: f64, y: f64) -> Point { Point { x, y } }
        },
    ];
    let err = ParsedModule::parse(&mut items).err().unwrap();
    assert!(err.to_string().contains("unknown constructor class"));
}

#[test]
fn borrowed_builtin_type_is_rejected() {
    let mut items = vec![parse_quote! {
        #[sim_fn(name = "echo")]
        #[case(args = "((capture value String))", result = "String")]
        fn echo(value: &str) -> String { value.to_owned() }
    }];
    let err = ParsedModule::parse(&mut items).err().unwrap();
    assert!(err.to_string().contains("borrowed builtin types"));
}

#[test]
fn codec_marker_is_parsed_and_consumed() {
    let mut items = vec![parse_quote! {
        #[sim_codec(symbol = "codec/mock", decode = "decode_mock", encode = "encode_mock")]
        fn mock_codec() {}
    }];
    let parsed = ParsedModule::parse(&mut items).unwrap();
    assert_eq!(parsed.codecs.len(), 1);
    assert_eq!(parsed.codecs[0].symbol, "codec/mock");
    assert_eq!(parsed.codecs[0].decode_ident, "decode_mock");
    assert_eq!(parsed.codecs[0].encode_ident, "encode_mock");
    assert!(parsed.cleaned_items.is_empty());
}

#[test]
fn macro_marker_is_parsed_and_consumed() {
    let mut items = vec![parse_quote! {
        #[sim_macro(symbol = "standard/proof-quote", expand = "expand_proof_quote")]
        fn proof_quote_macro() {}
    }];
    let parsed = ParsedModule::parse(&mut items).unwrap();
    assert_eq!(parsed.macros.len(), 1);
    assert_eq!(parsed.macros[0].symbol, "standard/proof-quote");
    assert_eq!(parsed.macros[0].expand_ident, "expand_proof_quote");
    assert!(parsed.cleaned_items.is_empty());
}

#[test]
fn number_domain_marker_is_parsed_and_consumed() {
    let mut items = vec![parse_quote! {
        #[sim_number_domain(
            symbol = "numbers/f64",
            parse = "parse_f64",
            encode = "encode_f64",
            add = "add_f64"
        )]
        fn f64_domain() {}
    }];
    let parsed = ParsedModule::parse(&mut items).unwrap();
    assert_eq!(parsed.number_domains.len(), 1);
    assert_eq!(parsed.number_domains[0].symbol, "numbers/f64");
    assert_eq!(parsed.number_domains[0].parse_ident, "parse_f64");
    assert_eq!(parsed.number_domains[0].encode_ident, "encode_f64");
    assert_eq!(
        parsed.number_domains[0]
            .add_ident
            .as_ref()
            .unwrap()
            .to_string(),
        "add_f64"
    );
    assert!(parsed.cleaned_items.is_empty());
}

#[test]
fn site_marker_is_parsed_and_consumed() {
    let mut items = vec![parse_quote! {
        #[sim_site(symbol = "model/local", realize = "realize_local")]
        fn local_site() {}
    }];
    let parsed = ParsedModule::parse(&mut items).unwrap();
    assert_eq!(parsed.sites.len(), 1);
    assert_eq!(parsed.sites[0].symbol, "model/local");
    assert_eq!(parsed.sites[0].realize_ident, "realize_local");
    assert!(parsed.cleaned_items.is_empty());
}

#[test]
fn site_marker_emits_site_manifest_export() {
    let mut items = vec![parse_quote! {
        #[sim_site(symbol = "model/local", realize = "realize_local")]
        fn local_site() {}
    }];
    let parsed = ParsedModule::parse(&mut items).unwrap();
    let vis = parse_quote!(pub);
    let generated = parsed
        .expand(&vis, &format_ident!("SiteLib"), "site-lib", "0.1.0", false)
        .unwrap()
        .to_string();

    assert!(generated.contains("Export :: Site"));
    assert!(generated.contains("Symbol :: qualified (\"model\" , \"local\")"));
}

#[test]
fn sexp_parser_handles_capture_shapes() {
    let parsed = parse_sexp("((capture x Number) (capture y String))").unwrap();
    assert!(matches!(parsed, Sexp::List(items) if items.len() == 2));
}

#[test]
fn camel_case_normalizes_module_names() {
    assert_eq!(camel_case("utility_tools"), "UtilityTools");
    assert_eq!(camel_case("m12"), "M12");
}
