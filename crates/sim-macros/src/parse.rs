//! Parses macro input into the intermediate Parsed* model.

use quote::format_ident;
use syn::{Ident, Item, LitStr};

mod markers;
mod module;
mod types;

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

impl ParsedFunction {
    pub(crate) fn class_id_args(&self) -> Vec<Ident> {
        unique_class_id_idents(&self.inputs)
    }
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
