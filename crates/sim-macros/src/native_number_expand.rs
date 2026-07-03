//! Native number-domain ABI dispatch generation.

use crate::parse::ParsedNumberDomain;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Ident, LitStr};

pub(crate) fn native_number_domain_call_dispatch(
    number_domains: &[ParsedNumberDomain],
) -> TokenStream2 {
    let arms = number_domains.iter().flat_map(number_domain_dispatch_arms);
    let result_adapter = if number_domains.is_empty() {
        TokenStream2::new()
    } else {
        quote! {
            trait NativeNumberDomainOptionalResult<T> {
                fn into_native_number_domain_optional_result(self) -> ::sim::kernel::Result<Option<T>>;
            }

            impl NativeNumberDomainOptionalResult<::sim::kernel::Expr>
                for Option<::sim::kernel::Expr>
            {
                fn into_native_number_domain_optional_result(
                    self,
                ) -> ::sim::kernel::Result<Option<::sim::kernel::Expr>> {
                    Ok(self)
                }
            }

            impl NativeNumberDomainOptionalResult<::sim::kernel::Expr>
                for ::sim::kernel::Result<Option<::sim::kernel::Expr>>
            {
                fn into_native_number_domain_optional_result(
                    self,
                ) -> ::sim::kernel::Result<Option<::sim::kernel::Expr>> {
                    self
                }
            }

            trait NativeNumberDomainExprResult {
                fn into_native_number_domain_expr_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr>;
            }

            impl NativeNumberDomainExprResult for ::sim::kernel::Expr {
                fn into_native_number_domain_expr_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                    Ok(self)
                }
            }

            impl NativeNumberDomainExprResult for ::sim::kernel::Result<::sim::kernel::Expr> {
                fn into_native_number_domain_expr_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                    self
                }
            }
        }
    };

    quote! {
        #result_adapter

        fn native_number_domain_call(
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

fn number_domain_dispatch_arms(domain: &ParsedNumberDomain) -> Vec<TokenStream2> {
    let parse_op = LitStr::new(
        &format!("{}/parse-literal", domain.symbol),
        Span::call_site(),
    );
    let encode_op = LitStr::new(
        &format!("{}/encode-literal", domain.symbol),
        Span::call_site(),
    );
    let parse_ident = &domain.parse_ident;
    let encode_ident = &domain.encode_ident;
    let mut arms = vec![
        quote! {
            #parse_op => {
                let text = match expr {
                    ::sim::kernel::Expr::String(text) => text.clone(),
                    other => {
                        return Err(::sim::kernel::Error::HostError(format!(
                            "{} expected literal text, got {:?}",
                            #parse_op,
                            other,
                        )));
                    }
                };
                let parsed = NativeNumberDomainOptionalResult::into_native_number_domain_optional_result(
                    super::#parse_ident(text),
                )?;
                Ok(Some(parsed.unwrap_or(::sim::kernel::Expr::Nil)))
            }
        },
        quote! {
            #encode_op => {
                let encoded = NativeNumberDomainOptionalResult::into_native_number_domain_optional_result(
                    super::#encode_ident(expr.clone()),
                )?;
                Ok(Some(encoded.unwrap_or(::sim::kernel::Expr::Nil)))
            }
        },
    ];

    arms.extend(binary_number_domain_arms(
        domain,
        "add",
        domain.add_ident.as_ref(),
    ));
    arms.extend(binary_number_domain_arms(
        domain,
        "sub",
        domain.sub_ident.as_ref(),
    ));
    arms.extend(binary_number_domain_arms(
        domain,
        "mul",
        domain.mul_ident.as_ref(),
    ));
    arms.extend(binary_number_domain_arms(
        domain,
        "div",
        domain.div_ident.as_ref(),
    ));
    arms.extend(unary_number_domain_arms(
        domain,
        "neg",
        domain.neg_ident.as_ref(),
    ));
    arms.extend(reduction_number_domain_arms(
        domain,
        "sum",
        domain.sum_ident.as_ref(),
    ));
    arms.extend(reduction_number_domain_arms(
        domain,
        "product",
        domain.product_ident.as_ref(),
    ));
    arms
}

fn binary_number_domain_arms(
    domain: &ParsedNumberDomain,
    op: &str,
    ident: Option<&Ident>,
) -> Option<TokenStream2> {
    let ident = ident?;
    let op_lit = LitStr::new(&format!("{}/{op}", domain.symbol), Span::call_site());
    Some(quote! {
        #op_lit => {
            let ::sim::kernel::Expr::List(items) = expr else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expected an argument list",
                    #op_lit,
                )));
            };
            let [left, right] = items.as_slice() else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expects exactly two arguments",
                    #op_lit,
                )));
            };
            let result = NativeNumberDomainExprResult::into_native_number_domain_expr_result(
                super::#ident(left.clone(), right.clone()),
            )?;
            Ok(Some(result))
        }
    })
}

fn unary_number_domain_arms(
    domain: &ParsedNumberDomain,
    op: &str,
    ident: Option<&Ident>,
) -> Option<TokenStream2> {
    let ident = ident?;
    let op_lit = LitStr::new(&format!("{}/{op}", domain.symbol), Span::call_site());
    Some(quote! {
        #op_lit => {
            let ::sim::kernel::Expr::List(items) = expr else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expected an argument list",
                    #op_lit,
                )));
            };
            let [operand] = items.as_slice() else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expects exactly one argument",
                    #op_lit,
                )));
            };
            let result = NativeNumberDomainExprResult::into_native_number_domain_expr_result(
                super::#ident(operand.clone()),
            )?;
            Ok(Some(result))
        }
    })
}

fn reduction_number_domain_arms(
    domain: &ParsedNumberDomain,
    op: &str,
    ident: Option<&Ident>,
) -> Option<TokenStream2> {
    let ident = ident?;
    let op_lit = LitStr::new(&format!("{}/{op}", domain.symbol), Span::call_site());
    Some(quote! {
        #op_lit => {
            let ::sim::kernel::Expr::List(items) = expr else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expected an argument list",
                    #op_lit,
                )));
            };
            let result = NativeNumberDomainExprResult::into_native_number_domain_expr_result(
                super::#ident(items.clone()),
            )?;
            Ok(Some(result))
        }
    })
}
