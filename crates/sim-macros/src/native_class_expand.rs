//! Native class ABI dispatch generation.

use crate::function_expand::{expr_binding_tokens, field_expr_tokens};
use crate::parse::{ParsedClass, ParsedFunction, ParsedFunctionKind, ParsedType};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::LitStr;

pub(crate) fn native_class_call_dispatch(
    classes: &[ParsedClass],
    functions: &[ParsedFunction],
) -> TokenStream2 {
    let constructor_arms = classes
        .iter()
        .filter_map(|class| constructor_dispatch_arm(class, functions));
    let member_arms = classes.iter().flat_map(member_dispatch_arms);

    quote! {
        fn native_class_call(
            function: &str,
            expr: &::sim::kernel::Expr,
        ) -> ::sim::kernel::Result<Option<::sim::kernel::Expr>> {
            match function {
                #(#constructor_arms,)*
                #(#member_arms,)*
                _ => Ok(None),
            }
        }
    }
}

pub(crate) fn native_class_validation_error(
    classes: &[ParsedClass],
    functions: &[ParsedFunction],
) -> Option<String> {
    for class in classes {
        for field in &class.fields {
            if matches!(
                &field.ty,
                ParsedType::RefClass(_) | ParsedType::OwnedClass(_)
            ) {
                return Some(format!(
                    "native_export = true supports class exports with scalar or Expr fields; field {}.{} uses a generated class type",
                    class.rust_ident, field.ident
                ));
            }
        }

        let constructor = functions.iter().find(|function| {
            matches!(
                &function.kind,
                ParsedFunctionKind::Constructor { class_ident } if class_ident == &class.rust_ident
            )
        });
        let Some(constructor) = constructor else {
            return Some(format!(
                "native_export = true class {} needs a matching #[sim_constructor]",
                class.rust_ident
            ));
        };
        for (_, ty) in &constructor.inputs {
            if matches!(ty, ParsedType::RefClass(_) | ParsedType::OwnedClass(_)) {
                return Some(format!(
                    "native_export = true constructor {} uses a generated class argument",
                    constructor.rust_ident
                ));
            }
        }
        if !matches!(
            &constructor.output,
            Some(ParsedType::OwnedClass(output)) if output == &class.rust_ident
        ) {
            return Some(format!(
                "native_export = true constructor {} must return {}",
                constructor.rust_ident, class.rust_ident
            ));
        }
    }
    None
}

fn constructor_dispatch_arm(
    class: &ParsedClass,
    functions: &[ParsedFunction],
) -> Option<TokenStream2> {
    let constructor = functions.iter().find(|function| {
        matches!(
            &function.kind,
            ParsedFunctionKind::Constructor { class_ident } if class_ident == &class.rust_ident
        )
    })?;
    let op_lit = LitStr::new(&format!("{}/new", class.lisp_name), Span::call_site());
    let class_symbol = LitStr::new(&class.lisp_name, Span::call_site());
    let expected_len = constructor.inputs.len();
    let arg_bindings = constructor
        .inputs
        .iter()
        .enumerate()
        .map(|(index, (name, ty))| expr_binding_tokens(index, name, ty));
    let call_args = constructor.inputs.iter().map(|(name, _)| quote!(#name));
    let constructor_ident = &constructor.rust_ident;
    let field_entries = class.fields.iter().map(|field| {
        let field_name = LitStr::new(&field.ident.to_string(), Span::call_site());
        let ident = &field.ident;
        let expr = field_expr_tokens(&field.ty, quote!(result.#ident));
        quote! {
            (
                ::sim::kernel::Symbol::new(#field_name),
                #expr,
            )
        }
    });

    Some(quote! {
        #op_lit => {
            let args = match expr {
                ::sim::kernel::Expr::List(items) => items.clone(),
                _ => {
                    return Err(::sim::kernel::Error::HostError(format!(
                        "{} expected an argument list",
                        #op_lit,
                    )));
                }
            };
            if args.len() != #expected_len {
                return Err(::sim::kernel::Error::Eval(format!(
                    "{} expects {} args, got {}",
                    #op_lit,
                    #expected_len,
                    args.len(),
                )));
            }
            #(#arg_bindings)*
            let result = super::#constructor_ident(#(#call_args),*);
            Ok(Some(::sim::shape::ObjectExpr {
                class: ::sim::kernel::Symbol::new(#class_symbol),
                fields: vec![#(#field_entries),*],
            }.to_expr()))
        }
    })
}

fn member_dispatch_arms(class: &ParsedClass) -> Vec<TokenStream2> {
    class
        .fields
        .iter()
        .flat_map(|field| {
            let slash_op = LitStr::new(
                &format!("{}/{}", class.lisp_name, field.ident),
                Span::call_site(),
            );
            let dot_op = LitStr::new(
                &format!("{}.{}", class.lisp_name, field.ident),
                Span::call_site(),
            );
            [
                member_dispatch_arm(class, field, slash_op),
                member_dispatch_arm(class, field, dot_op),
            ]
        })
        .collect()
}

fn member_dispatch_arm(
    class: &ParsedClass,
    field: &crate::parse::ParsedField,
    op_lit: LitStr,
) -> TokenStream2 {
    let class_symbol = LitStr::new(&class.lisp_name, Span::call_site());
    let field_symbol = LitStr::new(&field.ident.to_string(), Span::call_site());
    quote! {
        #op_lit => {
            let ::sim::kernel::Expr::List(items) = expr else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expected an argument list",
                    #op_lit,
                )));
            };
            let [instance] = items.as_slice() else {
                return Err(::sim::kernel::Error::HostError(format!(
                    "{} expects exactly one instance argument",
                    #op_lit,
                )));
            };
            let object = ::sim::shape::ObjectExpr::parse(instance).ok_or_else(|| {
                ::sim::kernel::Error::TypeMismatch {
                    expected: "object",
                    found: "non-object",
                }
            })?;
            let expected_class = ::sim::kernel::Symbol::new(#class_symbol);
            if object.class != expected_class {
                return Err(::sim::kernel::Error::TypeMismatch {
                    expected: #class_symbol,
                    found: "different class",
                });
            }
            let field = ::sim::kernel::Symbol::new(#field_symbol);
            let value = object.field(&field).cloned().ok_or_else(|| {
                ::sim::kernel::Error::UnknownSymbol { symbol: field }
            })?;
            Ok(Some(value))
        }
    }
}
