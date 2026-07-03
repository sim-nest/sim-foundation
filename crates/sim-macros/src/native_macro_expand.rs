//! Native macro ABI dispatch generation.

use crate::parse::ParsedMacro;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::LitStr;

pub(crate) fn native_macro_call_dispatch(macros: &[ParsedMacro]) -> TokenStream2 {
    let arms = macros.iter().map(macro_dispatch_arm);
    let result_adapter = if macros.is_empty() {
        TokenStream2::new()
    } else {
        quote! {
            trait NativeMacroResult {
                fn into_native_macro_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr>;
            }

            impl NativeMacroResult for ::sim::kernel::Expr {
                fn into_native_macro_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                    Ok(self)
                }
            }

            impl NativeMacroResult for ::sim::kernel::Result<::sim::kernel::Expr> {
                fn into_native_macro_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                    self
                }
            }
        }
    };

    quote! {
        #result_adapter

        fn native_macro_call(
            function: &str,
            expr: &::sim::kernel::Expr,
        ) -> ::sim::kernel::Result<Option<::sim::kernel::Expr>> {
            match function {
                #(#arms,)*
                _ => Ok(None),
            }
        }
    }
}

fn macro_dispatch_arm(mac: &ParsedMacro) -> TokenStream2 {
    let op_lit = LitStr::new(&format!("{}/expand", mac.symbol), Span::call_site());
    let expand_ident = &mac.expand_ident;
    quote! {
        #op_lit => {
            let expanded = NativeMacroResult::into_native_macro_result(
                super::#expand_ident(expr.clone()),
            )?;
            Ok(Some(expanded))
        }
    }
}
