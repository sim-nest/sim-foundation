//! Code generation for function, class, and method macro expansions.

use crate::parse::{
    ParsedClass, ParsedFunction, ParsedFunctionKind, ParsedType, unique_class_id_idents,
};
use crate::shape::{class_shape_tokens, parse_shape_literal};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Ident, LitStr};

pub(crate) fn expand_class_impl(
    class: &ParsedClass,
    functions: &[ParsedFunction],
) -> syn::Result<TokenStream2> {
    let wrapper_ident = &class.wrapper_ident;
    let rust_ident = &class.rust_ident;
    let class_symbol = LitStr::new(&class.lisp_name, Span::call_site());
    let Some(constructor_builder) = functions.iter().find_map(|function| match &function.kind {
        ParsedFunctionKind::Constructor { class_ident, .. } if class_ident == rust_ident => {
            Some(format_ident!(
                "build_{}_constructor",
                function.rust_ident.to_string().to_lowercase()
            ))
        }
        _ => None,
    }) else {
        return Err(syn::Error::new(
            class.rust_ident.span(),
            format!(
                "lisp class {} must have a matching #[sim_constructor]",
                class.lisp_name
            ),
        ));
    };
    let class_builder = format_ident!("build_{}_class", rust_ident.to_string().to_lowercase());
    let member_symbols = class.fields.iter().map(|field| {
        let name = LitStr::new(&field.ident.to_string(), Span::call_site());
        quote!(::sim::kernel::Symbol::new(#name))
    });
    let instance_shape = class_shape_tokens(class.shape_literal.as_ref(), &class.lisp_name)?;
    let as_expr_entries = class.fields.iter().map(|field| {
        let name_ident = &field.ident;
        let name = LitStr::new(&field.ident.to_string(), Span::call_site());
        let expr = field_expr_tokens(&field.ty, quote!(self.0.#name_ident));
        quote! {
            (
                ::sim::kernel::Symbol::new(#name),
                #expr,
            )
        }
    });
    let field_exprs = class.fields.iter().map(|field| {
        let name_ident = &field.ident;
        let name = LitStr::new(&field.ident.to_string(), Span::call_site());
        let expr = field_expr_tokens(&field.ty, quote!(self.0.#name_ident));
        quote! {
            (
                ::sim::kernel::Symbol::new(#name),
                cx.factory().expr(#expr)?,
            )
        }
    });
    let constructor_args = class.fields.iter().map(|field| {
        let name_ident = &field.ident;
        field_expr_tokens(&field.ty, quote!(self.0.#name_ident))
    });
    Ok(quote! {
            #[derive(Clone)]
            struct #wrapper_ident(super::#rust_ident);

            impl #wrapper_ident {
                fn new(inner: super::#rust_ident) -> Self {
                    Self(inner)
                }
            }

            #[allow(deprecated)]
            impl ::sim::kernel::Object for #wrapper_ident {


                fn display(&self, _cx: &mut ::sim::kernel::Cx) -> ::sim::kernel::Result<String> {
                    Ok(format!("#<instance {}>", #class_symbol))
                }



                fn as_any(&self) -> &dyn std::any::Any {
                    self
                }
            }

    impl ::sim::kernel::ObjectCompat for #wrapper_ident {            fn class(&self, cx: &mut ::sim::kernel::Cx) -> ::sim::kernel::Result<::sim::kernel::ClassRef> {
                    if let Some(value) = cx.registry().class_by_symbol(&::sim::kernel::Symbol::new(#class_symbol)) {
                        return Ok(value.clone());
                    }
                    cx.factory().class_stub(::sim::kernel::ClassId(0), ::sim::kernel::Symbol::new(#class_symbol))
                }
                fn as_expr(&self, _cx: &mut ::sim::kernel::Cx) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                    Ok(::sim::shape::ObjectExpr {
                        class: ::sim::kernel::Symbol::new(#class_symbol),
                        fields: vec![#(#as_expr_entries),*],
                    }.to_expr())
                }
                fn as_table(&self, cx: &mut ::sim::kernel::Cx) -> ::sim::kernel::Result<::sim::kernel::Value> {
                    cx.factory().table(vec![#(#field_exprs),*])
                }
                fn as_object_encoder(&self) -> Option<&dyn ::sim::kernel::ObjectEncode> {
                    Some(self)
                }
    }

            #[allow(deprecated)]
            impl ::sim::kernel::ObjectEncode for #wrapper_ident {
                fn object_encoding(
                    &self,
                    _cx: &mut ::sim::kernel::Cx,
                ) -> ::sim::kernel::Result<::sim::kernel::ObjectEncoding> {
                    Ok(::sim::kernel::ObjectEncoding::Constructor {
                        class: ::sim::kernel::Symbol::new(#class_symbol),
                        args: vec![#(#constructor_args),*],
                    })
                }
            }

            pub(super) fn #class_builder(cx: &mut ::sim::kernel::LoadCx) -> ::sim::kernel::Result<::sim::classes::NativeClass> {
                let constructor = #constructor_builder(cx)?;
                Ok(::sim::classes::NativeClass::new(
                    cx.fresh_class_id(),
                    ::sim::kernel::Symbol::new(#class_symbol),
                    constructor,
                    Some(#instance_shape),
                    vec![#(#member_symbols),*],
                ))
            }
        })
}

pub(crate) fn expand_function_impl(
    function: &ParsedFunction,
    classes: &[ParsedClass],
) -> syn::Result<TokenStream2> {
    let rust_ident = &function.rust_ident;
    let adapter_ident = format_ident!("{}_adapter", rust_ident.to_string().to_lowercase());
    let builder_ident = match function.kind {
        ParsedFunctionKind::Constructor { .. } => {
            format_ident!(
                "build_{}_constructor",
                rust_ident.to_string().to_lowercase()
            )
        }
        ParsedFunctionKind::Function { .. } => {
            format_ident!("build_{}_function", rust_ident.to_string().to_lowercase())
        }
    };
    let symbol_name = match &function.kind {
        ParsedFunctionKind::Constructor { .. } => rust_ident.to_string(),
        ParsedFunctionKind::Function { lisp_name } => lisp_name.clone(),
    };
    let symbol_lit = LitStr::new(&symbol_name, Span::call_site());

    let class_id_idents = unique_class_id_idents(&function.inputs);
    let class_id_params = class_id_idents
        .iter()
        .map(|ident| quote!(#ident: ::sim::kernel::ClassId));

    let arg_bindings = function
        .inputs
        .iter()
        .enumerate()
        .map(|(index, (name, ty))| binding_tokens(index, name, ty));

    let call_args = function.inputs.iter().map(|(name, _)| quote!(#name));
    let result_wrap = result_wrap_tokens(function.output.as_ref(), classes, quote!(result));
    let cases = function
        .cases
        .iter()
        .enumerate()
        .map(|(index, case)| -> syn::Result<TokenStream2> {
            let case_name = LitStr::new(&format!("{symbol_name}/case-{index}"), Span::call_site());
            let args_shape = parse_shape_literal(&case.args)?;
            let result_shape = case
                .result
                .as_ref()
                .map(parse_shape_literal)
                .transpose()?
                .unwrap_or_else(|| quote!(::std::sync::Arc::new(::sim::shape::AnyShape)));
            let demands = demands_tokens(&function.inputs);
            Ok(quote! {
                ::sim::functions::FunctionCase {
                    id: cx.fresh_case_id(),
                    name: ::sim::kernel::Symbol::new(#case_name),
                    args: #args_shape,
                    result: Some(#result_shape),
                    demand: vec![#(#demands),*],
                    priority: 0,
                    implementation: #adapter_ident,
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        #[allow(deprecated)]
        fn #adapter_ident(
            cx: &mut ::sim::kernel::Cx,
            prepared: &::sim::kernel::PreparedArgs,
            _bindings: ::sim::shape::Bindings,
        ) -> ::sim::kernel::Result<::sim::kernel::Value> {
            #(#arg_bindings)*
            let result = super::#rust_ident(#(#call_args),*);
            #result_wrap
        }

        pub(super) fn #builder_ident(
            cx: &mut ::sim::kernel::LoadCx,
            #(#class_id_params),*
        ) -> ::sim::kernel::Result<::sim::functions::FunctionObject> {
            Ok(::sim::functions::FunctionObject::new(
                cx.fresh_function_id(),
                ::sim::kernel::Symbol::new(#symbol_lit),
                vec![#(#cases),*],
            ))
        }
    })
}

fn binding_tokens(index: usize, name: &Ident, ty: &ParsedType) -> TokenStream2 {
    let index_lit = syn::Index::from(index);
    let missing = format!("missing prepared arg {index}");
    match ty {
        ParsedType::F64 => quote! {
            let #name = {
                let value = prepared.get(#index_lit).ok_or_else(|| {
                    ::sim::kernel::Error::Eval(#missing.to_owned())
                })?;
                match value.object().as_expr(cx)? {
                    ::sim::kernel::Expr::Number(number) => number.canonical.parse::<f64>().map_err(|err| {
                        ::sim::kernel::Error::HostError(format!("failed to parse {} as f64: {err}", stringify!(#name)))
                    })?,
                    _ => return Err(::sim::kernel::Error::TypeMismatch {
                        expected: "number",
                        found: "non-number",
                    }),
                }
            };
        },
        ParsedType::Bool => quote! {
            let #name = {
                let value = prepared.get(#index_lit).ok_or_else(|| {
                    ::sim::kernel::Error::Eval(#missing.to_owned())
                })?;
                match value.object().as_expr(cx)? {
                    ::sim::kernel::Expr::Bool(value) => value,
                    _ => return Err(::sim::kernel::Error::TypeMismatch {
                        expected: "bool",
                        found: "non-bool",
                    }),
                }
            };
        },
        ParsedType::String => quote! {
            let #name = {
                let value = prepared.get(#index_lit).ok_or_else(|| {
                    ::sim::kernel::Error::Eval(#missing.to_owned())
                })?;
                match value.object().as_expr(cx)? {
                    ::sim::kernel::Expr::String(value) => value,
                    _ => return Err(::sim::kernel::Error::TypeMismatch {
                        expected: "string",
                        found: "non-string",
                    }),
                }
            };
        },
        ParsedType::Symbol => quote! {
            let #name = {
                let value = prepared.get(#index_lit).ok_or_else(|| {
                    ::sim::kernel::Error::Eval(#missing.to_owned())
                })?;
                match value.object().as_expr(cx)? {
                    ::sim::kernel::Expr::Symbol(value) => value,
                    _ => return Err(::sim::kernel::Error::TypeMismatch {
                        expected: "symbol",
                        found: "non-symbol",
                    }),
                }
            };
        },
        ParsedType::Expr => quote! {
            let #name = {
                let value = prepared.get(#index_lit).ok_or_else(|| {
                    ::sim::kernel::Error::Eval(#missing.to_owned())
                })?;
                value.object().as_expr(cx)?
            };
        },
        ParsedType::RefClass(class) => {
            let wrapper = format_ident!("__Lisp{}Value", class);
            quote! {
                let #name = {
                    let value = prepared.get(#index_lit).ok_or_else(|| {
                        ::sim::kernel::Error::Eval(#missing.to_owned())
                    })?;
                    &value
                        .object()
                        .downcast_ref::<#wrapper>()
                        .ok_or(::sim::kernel::Error::TypeMismatch {
                            expected: stringify!(#class),
                            found: "non-generated class wrapper",
                        })?
                        .0
                };
            }
        }
        ParsedType::OwnedClass(class) => {
            let wrapper = format_ident!("__Lisp{}Value", class);
            quote! {
                let #name = {
                    let value = prepared.get(#index_lit).ok_or_else(|| {
                        ::sim::kernel::Error::Eval(#missing.to_owned())
                    })?;
                    value
                        .object()
                        .downcast_ref::<#wrapper>()
                        .ok_or(::sim::kernel::Error::TypeMismatch {
                            expected: stringify!(#class),
                            found: "non-generated class wrapper",
                        })?
                        .0
                        .clone()
                };
            }
        }
    }
}

pub(crate) fn expr_binding_tokens(index: usize, name: &Ident, ty: &ParsedType) -> TokenStream2 {
    let index_lit = syn::Index::from(index);
    let missing = format!("missing expr arg {index}");
    match ty {
        ParsedType::F64 => quote! {
            let #name = match args.get(#index_lit).ok_or_else(|| {
                ::sim::kernel::Error::Eval(#missing.to_owned())
            })? {
                ::sim::kernel::Expr::Number(number) => number.canonical.parse::<f64>().map_err(|err| {
                    ::sim::kernel::Error::HostError(format!("failed to parse {} as f64: {err}", stringify!(#name)))
                })?,
                _ => return Err(::sim::kernel::Error::TypeMismatch {
                    expected: "number",
                    found: "non-number",
                }),
            };
        },
        ParsedType::Bool => quote! {
            let #name = match args.get(#index_lit).ok_or_else(|| {
                ::sim::kernel::Error::Eval(#missing.to_owned())
            })? {
                ::sim::kernel::Expr::Bool(value) => *value,
                _ => return Err(::sim::kernel::Error::TypeMismatch {
                    expected: "bool",
                    found: "non-bool",
                }),
            };
        },
        ParsedType::String => quote! {
            let #name = match args.get(#index_lit).ok_or_else(|| {
                ::sim::kernel::Error::Eval(#missing.to_owned())
            })? {
                ::sim::kernel::Expr::String(value) => value.clone(),
                _ => return Err(::sim::kernel::Error::TypeMismatch {
                    expected: "string",
                    found: "non-string",
                }),
            };
        },
        ParsedType::Symbol => quote! {
            let #name = match args.get(#index_lit).ok_or_else(|| {
                ::sim::kernel::Error::Eval(#missing.to_owned())
            })? {
                ::sim::kernel::Expr::Symbol(value) => value.clone(),
                _ => return Err(::sim::kernel::Error::TypeMismatch {
                    expected: "symbol",
                    found: "non-symbol",
                }),
            };
        },
        ParsedType::Expr => quote! {
            let #name = args.get(#index_lit).ok_or_else(|| {
                ::sim::kernel::Error::Eval(#missing.to_owned())
            })?.clone();
        },
        ParsedType::RefClass(_) | ParsedType::OwnedClass(_) => {
            quote! {
                return Err(::sim::kernel::Error::HostError(
                    "native ABI expr dispatch does not support generated class arguments".to_owned()
                ));
            }
        }
    }
}

fn result_wrap_tokens(
    output: Option<&ParsedType>,
    _classes: &[ParsedClass],
    value: TokenStream2,
) -> TokenStream2 {
    match output {
        Some(ParsedType::F64) => quote! {
            cx.factory().number_literal(
                ::sim::kernel::Symbol::qualified("numbers", "f64"),
                #value.to_string(),
            )
        },
        Some(ParsedType::Bool) => quote!(cx.factory().bool(#value)),
        Some(ParsedType::String) => quote!(cx.factory().string(#value)),
        Some(ParsedType::Symbol) => quote!(cx.factory().symbol(#value)),
        Some(ParsedType::Expr) => quote!(cx.factory().expr(#value)),
        Some(ParsedType::OwnedClass(class)) => {
            let wrapper = format_ident!("__Lisp{}Value", class);
            quote!(::sim::kernel::Factory::opaque(
                &::sim::kernel::DefaultFactory,
                ::std::sync::Arc::new(#wrapper::new(#value)),
            ))
        }
        Some(ParsedType::RefClass(_)) => quote! {
            Err(::sim::kernel::Error::HostError("unsupported borrowed return type".to_owned()))
        },
        None => quote!(cx.factory().nil()),
    }
}

pub(crate) fn expr_result_wrap_tokens(
    output: Option<&ParsedType>,
    value: TokenStream2,
) -> TokenStream2 {
    match output {
        Some(ParsedType::F64) => quote! {
            ::sim::kernel::Expr::Number(::sim::kernel::NumberLiteral {
                domain: ::sim::kernel::Symbol::qualified("numbers", "f64"),
                canonical: #value.to_string(),
            })
        },
        Some(ParsedType::Bool) => quote!(::sim::kernel::Expr::Bool(#value)),
        Some(ParsedType::String) => quote!(::sim::kernel::Expr::String(#value)),
        Some(ParsedType::Symbol) => quote!(::sim::kernel::Expr::Symbol(#value)),
        Some(ParsedType::Expr) => quote!(#value),
        Some(ParsedType::RefClass(_)) | Some(ParsedType::OwnedClass(_)) => quote! {
            return Err(::sim::kernel::Error::HostError(
                "native ABI expr dispatch does not support generated class returns".to_owned()
            ));
        },
        None => quote!(::sim::kernel::Expr::Nil),
    }
}

fn demands_tokens(inputs: &[(Ident, ParsedType)]) -> Vec<TokenStream2> {
    inputs
        .iter()
        .map(|(_, ty)| match ty {
            ParsedType::F64 | ParsedType::Bool | ParsedType::String | ParsedType::Symbol => {
                quote!(::sim::kernel::Demand::Value)
            }
            ParsedType::Expr => quote!(::sim::kernel::Demand::Expr),
            ParsedType::RefClass(class) | ParsedType::OwnedClass(class) => {
                let id_ident =
                    format_ident!("__lisp_class_id_{}", class.to_string().to_lowercase());
                quote!(::sim::kernel::Demand::Class(#id_ident))
            }
        })
        .collect()
}

pub(crate) fn field_expr_tokens(ty: &ParsedType, access: TokenStream2) -> TokenStream2 {
    match ty {
        ParsedType::F64 => quote! {
            ::sim::kernel::Expr::Number(::sim::kernel::NumberLiteral {
                domain: ::sim::kernel::Symbol::qualified("numbers", "f64"),
                canonical: #access.to_string(),
            })
        },
        ParsedType::Bool => quote!(::sim::kernel::Expr::Bool(#access)),
        ParsedType::String => quote!(::sim::kernel::Expr::String(#access.clone())),
        ParsedType::Symbol => quote!(::sim::kernel::Expr::Symbol(#access.clone())),
        ParsedType::Expr => quote!(#access.clone()),
        ParsedType::RefClass(class) | ParsedType::OwnedClass(class) => {
            let wrapper = format_ident!("__Lisp{}Value", class);
            quote! {
                ::sim::kernel::Factory::opaque(
                    &::sim::kernel::DefaultFactory,
                    ::std::sync::Arc::new(#wrapper::new(#access.clone())),
                )?.object().as_expr(_cx)?
            }
        }
    }
}
