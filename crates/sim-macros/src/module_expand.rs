//! Code generation for module-level macro expansion.

use crate::function_expand::{expand_class_impl, expand_function_impl};
use crate::macro_expand::{macro_impls, macro_loads};
use crate::parse::{ParsedFunctionKind, ParsedModule};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Ident, LitStr, Visibility};

mod native_export;

impl ParsedModule {
    pub(crate) fn expand(
        &self,
        _module_vis: &Visibility,
        lib_ident: &Ident,
        lib_id: &str,
        version: &str,
        native_export: bool,
    ) -> syn::Result<TokenStream2> {
        let manifest_exports = self.manifest_exports();
        let generated_impls = self.class_impls()?;
        let class_loads = self.class_loads(version);
        let function_impls = self.function_impls()?;
        let function_loads = self.function_loads();
        let macro_impls = macro_impls(&self.macros);
        let macro_loads = macro_loads(&self.macros);
        let native_export_impl = self.native_export_impl(lib_ident, native_export)?;
        let lib_id_lit = LitStr::new(lib_id, Span::call_site());
        let version_lit = LitStr::new(version, Span::call_site());

        Ok(quote! {
            pub struct #lib_ident;

            impl ::sim::kernel::Lib for #lib_ident {
                fn manifest(&self) -> ::sim::kernel::LibManifest {
                    ::sim::kernel::LibManifest {
                        id: ::sim::kernel::Symbol::new(#lib_id_lit),
                        version: ::sim::kernel::Version(#version_lit.to_owned()),
                        abi: ::sim::kernel::AbiVersion { major: 0, minor: 1 },
                        target: ::sim::kernel::LibTarget::HostRegistered,
                        requires: Vec::<::sim::kernel::Dependency>::new(),
                        capabilities: Vec::new(),
                        exports: vec![#(#manifest_exports),*],
                    }
                }

                fn load(
                    &self,
                    cx: &mut ::sim::kernel::LoadCx,
                    linker: &mut ::sim::kernel::Linker,
                ) -> ::sim::kernel::Result<()> {
                    #(#class_loads)*
                    #(#function_loads)*
                    #(#macro_loads)*
                    Ok(())
                }
            }

            mod generated {
                #(#generated_impls)*
                #(#function_impls)*
                #macro_impls
            }

            #native_export_impl
        })
    }

    fn manifest_exports(&self) -> Vec<TokenStream2> {
        let mut exports = Vec::new();
        for class in &self.classes {
            let class_symbol = LitStr::new(&class.lisp_name, Span::call_site());
            exports.push(quote! {
                ::sim::kernel::Export::Class {
                    symbol: ::sim::kernel::Symbol::new(#class_symbol),
                    class_id: None,
                }
            });
            exports.push(quote! {
                ::sim::kernel::Export::Shape {
                    symbol: ::sim::kernel::Symbol::qualified(#class_symbol, "constructor-shape"),
                    shape_id: None,
                }
            });
            exports.push(quote! {
                ::sim::kernel::Export::Shape {
                    symbol: ::sim::kernel::Symbol::qualified(#class_symbol, "instance-shape"),
                    shape_id: None,
                }
            });
            for field in &class.fields {
                let field_name = LitStr::new(&field.ident.to_string(), Span::call_site());
                exports.push(quote! {
                    ::sim::kernel::Export::Function {
                        symbol: ::sim::kernel::Symbol::qualified(#class_symbol, #field_name),
                        function_id: None,
                    }
                });
            }
        }

        for function in &self.functions {
            let symbol = match &function.kind {
                ParsedFunctionKind::Constructor { .. } => function.rust_ident.to_string(),
                ParsedFunctionKind::Function { lisp_name } => lisp_name.clone(),
            };
            let symbol = LitStr::new(&symbol, Span::call_site());
            exports.push(quote! {
                ::sim::kernel::Export::Function {
                    symbol: ::sim::kernel::Symbol::new(#symbol),
                    function_id: None,
                }
            });
        }

        for mac in &self.macros {
            let symbol = symbol_expr_tokens(&mac.symbol);
            exports.push(quote! {
                ::sim::kernel::Export::Macro {
                    symbol: #symbol,
                    macro_id: None,
                }
            });
        }

        for codec in &self.codecs {
            let symbol = symbol_expr_tokens(&codec.symbol);
            exports.push(quote! {
                ::sim::kernel::Export::Codec {
                    symbol: #symbol,
                    codec_id: None,
                }
            });
        }

        for number_domain in &self.number_domains {
            let symbol = symbol_expr_tokens(&number_domain.symbol);
            exports.push(quote! {
                ::sim::kernel::Export::NumberDomain {
                    symbol: #symbol,
                    number_domain_id: None,
                }
            });
        }

        for site in &self.sites {
            let symbol = symbol_expr_tokens(&site.symbol);
            exports.push(quote! {
                ::sim::kernel::Export::Site {
                    symbol: #symbol,
                    runtime_id: None,
                }
            });
        }

        exports
    }

    fn class_impls(&self) -> syn::Result<Vec<TokenStream2>> {
        self.classes
            .iter()
            .map(|class| expand_class_impl(class, &self.functions))
            .collect()
    }

    fn class_loads(&self, version: &str) -> Vec<TokenStream2> {
        self.classes
            .iter()
            .map(|class| {
                let builder = format_ident!(
                    "build_{}_class",
                    class.rust_ident.to_string().to_lowercase()
                );
                let id_ident = format_ident!(
                    "__lisp_class_id_{}",
                    class.rust_ident.to_string().to_lowercase()
                );
                let class_symbol = LitStr::new(&class.lisp_name, Span::call_site());
                let version_lit = LitStr::new(version, Span::call_site());
                quote! {
                    let class = generated::#builder(cx)?;
                    let #id_ident = class.id;
                    let class_lib = ::sim::classes::NativeClassLib::from_class(
                        ::sim::kernel::Symbol::qualified("generated", #class_symbol),
                        &class,
                        #version_lit,
                    );
                    ::sim::kernel::Lib::load(&class_lib, cx, linker)?;
                }
            })
            .collect()
    }

    fn function_impls(&self) -> syn::Result<Vec<TokenStream2>> {
        self.functions
            .iter()
            .map(|function| expand_function_impl(function, &self.classes))
            .collect()
    }

    fn function_loads(&self) -> Vec<TokenStream2> {
        self.functions
            .iter()
            .filter_map(|function| match function.kind {
                ParsedFunctionKind::Function { .. } => {
                    let builder = format_ident!(
                        "build_{}_function",
                        function.rust_ident.to_string().to_lowercase()
                    );
                    let symbol = match &function.kind {
                        ParsedFunctionKind::Function { lisp_name } => {
                            LitStr::new(lisp_name, Span::call_site())
                        }
                        ParsedFunctionKind::Constructor { .. } => unreachable!(),
                    };
                    let class_ids = function.class_id_args();
                    Some(quote! {
                        let function = generated::#builder(cx #(, #class_ids)*)?;
                        linker.function_value(
                            ::sim::kernel::Symbol::new(#symbol),
                            ::sim::kernel::Factory::opaque(
                                &::sim::kernel::DefaultFactory,
                                ::std::sync::Arc::new(function),
                            )?,
                        )?;
                    })
                }
                ParsedFunctionKind::Constructor { .. } => None,
            })
            .collect()
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
