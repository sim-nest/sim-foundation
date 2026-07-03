//! Proc-macro surface for authored SIM libraries.
//!
//! `#[sim_class]`, `#[sim_constructor]`, `#[sim_fn]`, `#[sim_macro]`,
//! `#[sim_codec]`, `#[sim_number_domain]`, `#[sim_site]`, `#[case]`, and
//! `#[shape]` are marker attributes. They do not transform the attached item on
//! their own. `#[sim_lib(...)]` scans an inline module for those markers,
//! validates the contracts, and generates the runtime registration and optional
//! native ABI glue.
//!
//! Invalid shape literals and missing generated-class constructors are rejected
//! during macro expansion rather than silently widening contracts.

#![forbid(unsafe_code)]
#![allow(deprecated)]
#![deny(missing_docs)]

mod attrs;
mod function_expand;
mod macro_expand;
mod module_expand;
mod native_class_expand;
mod native_macro_expand;
mod native_number_expand;
mod parse;
mod shape;

use attrs::{bool_attr, camel_case, string_attr};
use parse::ParsedModule;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{ItemMod, LitStr, Token, parse_macro_input, punctuated::Punctuated};

/// Expands an inline module into a SIM library.
///
/// Parses the module's `#[sim_class]`, `#[sim_constructor]`, `#[sim_fn]`,
/// `#[sim_macro]`, `#[sim_codec]`, `#[sim_number_domain]`, `#[sim_site]`,
/// `#[case]`, and `#[shape]` items and generates the library registration plus an
/// `__SIM_LIB_EXPANSION` snapshot.
/// Requires `id` and `version` attributes; `native_export` is optional. The
/// annotated module must be inline.
#[proc_macro_attribute]
pub fn sim_lib(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr =
        parse_macro_input!(attr with Punctuated::<syn::MetaNameValue, Token![,]>::parse_terminated);
    let mut module = parse_macro_input!(item as ItemMod);

    let lib_id = match string_attr(&attr, "id") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let version = match string_attr(&attr, "version") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let native_export = match bool_attr(&attr, "native_export") {
        Ok(value) => value,
        Err(err) => return err,
    };

    let Some((_, items)) = &mut module.content else {
        return syn::Error::new_spanned(&module, "#[sim_lib] requires an inline module")
            .to_compile_error()
            .into();
    };

    let parsed = match ParsedModule::parse(items) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error().into(),
    };

    let lib_ident = format_ident!("{}Lib", camel_case(&module.ident.to_string()));
    let generated = match parsed.expand(&module.vis, &lib_ident, &lib_id, &version, native_export) {
        Ok(generated) => generated,
        Err(err) => return err.to_compile_error().into(),
    };
    let expansion_text = generated.to_string();
    let expansion_text = LitStr::new(&expansion_text, Span::call_site());

    *items = parsed.cleaned_items;
    items.push(syn::parse_quote! {
        pub const __SIM_LIB_EXPANSION: &str = #expansion_text;
    });
    items.extend(
        syn::parse2::<syn::File>(generated)
            .expect("generated sim_lib items should parse")
            .items,
    );

    quote!(#module).into()
}

/// Marks a struct inside a `#[sim_lib]` module as a SIM class. It is consumed by
/// `#[sim_lib]`; applied on its own it leaves the item unchanged.
#[proc_macro_attribute]
pub fn sim_class(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a function inside a `#[sim_lib]` module as a class constructor. It is
/// consumed by `#[sim_lib]`; applied on its own it leaves the item unchanged.
#[proc_macro_attribute]
pub fn sim_constructor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a function inside a `#[sim_lib]` module as a SIM operation. It is
/// consumed by `#[sim_lib]`; applied on its own it leaves the item unchanged.
#[proc_macro_attribute]
pub fn sim_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a macro export inside a `#[sim_lib]` module. The marker names the macro
/// symbol plus the expansion function called by host registration and generated
/// native ABI dispatch.
#[proc_macro_attribute]
pub fn sim_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a codec export inside a `#[sim_lib]` module. The marker names the
/// codec symbol plus decoder and encoder functions that are called by generated
/// native ABI dispatch.
#[proc_macro_attribute]
pub fn sim_codec(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a number-domain export inside a `#[sim_lib]` module. The marker names
/// the domain symbol plus guest functions that are called by generated native
/// ABI dispatch for parsing, literal encoding, and optional scalar operations.
#[proc_macro_attribute]
pub fn sim_number_domain(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a site export inside a `#[sim_lib]` module. The marker names the site
/// symbol plus the function called by generated native ABI dispatch for
/// realization requests.
#[proc_macro_attribute]
pub fn sim_site(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a function inside a `#[sim_lib]` module as a test case. It is consumed
/// by `#[sim_lib]`; applied on its own it leaves the item unchanged.
#[proc_macro_attribute]
pub fn case(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Attaches a shape literal to an item inside a `#[sim_lib]` module. It is
/// consumed by `#[sim_lib]`; applied on its own it leaves the item unchanged.
#[proc_macro_attribute]
pub fn shape(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[cfg(test)]
mod tests;
