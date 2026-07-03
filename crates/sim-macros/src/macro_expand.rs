//! Host macro registration code generation.

use crate::parse::ParsedMacro;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::LitStr;

pub(crate) fn macro_loads(macros: &[ParsedMacro]) -> Vec<TokenStream2> {
    macros
        .iter()
        .map(|mac| {
            let builder = format_ident!(
                "build_{}_macro",
                mac.expand_ident.to_string().to_lowercase()
            );
            let symbol = symbol_expr_tokens(&mac.symbol);
            quote! {
                linker.macro_value(
                    #symbol,
                    generated::#builder(),
                )?;
            }
        })
        .collect()
}

pub(crate) fn macro_impls(macros: &[ParsedMacro]) -> TokenStream2 {
    if macros.is_empty() {
        return TokenStream2::new();
    }
    let builders = macros.iter().map(|mac| {
        let builder = format_ident!(
            "build_{}_macro",
            mac.expand_ident.to_string().to_lowercase()
        );
        let adapter = format_ident!(
            "{}_macro_adapter",
            mac.expand_ident.to_string().to_lowercase()
        );
        let expand_ident = &mac.expand_ident;
        let symbol = symbol_expr_tokens(&mac.symbol);
        quote! {
            fn #adapter(
                _cx: &mut ::sim::macros::MacroCx<'_>,
                input: ::sim::kernel::Expr,
                _captures: ::sim::shape::Bindings,
            ) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                GeneratedMacroResult::into_generated_macro_result(super::#expand_ident(input))
            }

            pub(super) fn #builder() -> ::sim::kernel::Value {
                let symbol = #symbol;
                let syntax_shape = ::sim::macros::list_macro_shape_with_rest(
                    symbol.clone(),
                    Vec::new(),
                    ::std::sync::Arc::new(::sim::shape::AnyShape),
                );
                ::sim::macros::macro_value(::std::sync::Arc::new(
                    ::sim::macros::NativeExprMacro::new(symbol, syntax_shape, #adapter),
                ))
            }
        }
    });

    quote! {
        trait GeneratedMacroResult {
            fn into_generated_macro_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr>;
        }

        impl GeneratedMacroResult for ::sim::kernel::Expr {
            fn into_generated_macro_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                Ok(self)
            }
        }

        impl GeneratedMacroResult for ::sim::kernel::Result<::sim::kernel::Expr> {
            fn into_generated_macro_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                self
            }
        }

        #(#builders)*
    }
}

fn symbol_expr_tokens(symbol: &str) -> TokenStream2 {
    if let Some((namespace, name)) = symbol.split_once('/') {
        let namespace = LitStr::new(namespace, Span::call_site());
        let name = LitStr::new(name, Span::call_site());
        quote!(::sim::kernel::Symbol::qualified(#namespace, #name))
    } else {
        let symbol = LitStr::new(symbol, Span::call_site());
        quote!(::sim::kernel::Symbol::new(#symbol))
    }
}
