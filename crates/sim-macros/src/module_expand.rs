//! Code generation for module-level macro expansion.

use crate::function_expand::{
    expand_class_impl, expand_function_impl, expr_binding_tokens, expr_result_wrap_tokens,
};
use crate::macro_expand::{macro_impls, macro_loads};
use crate::native_class_expand::{native_class_call_dispatch, native_class_validation_error};
use crate::native_macro_expand::native_macro_call_dispatch;
use crate::native_number_expand::native_number_domain_call_dispatch;
use crate::parse::{ParsedCodec, ParsedFunctionKind, ParsedModule, ParsedSite, ParsedType};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Ident, LitStr, Visibility};

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

    fn native_export_impl(&self, lib_ident: &Ident, enabled: bool) -> syn::Result<TokenStream2> {
        if !enabled {
            return Ok(TokenStream2::new());
        }
        if let Some(message) = self.native_export_validation_error() {
            let message = LitStr::new(&message, Span::call_site());
            return Ok(quote!(compile_error!(#message);));
        }
        let native_class_call_dispatch = native_class_call_dispatch(&self.classes, &self.functions);
        let native_macro_call_dispatch = native_macro_call_dispatch(&self.macros);
        let native_call_dispatch = self.native_call_dispatch();
        let native_codec_call_dispatch = self.native_codec_call_dispatch();
        let native_number_domain_call_dispatch =
            native_number_domain_call_dispatch(&self.number_domains);
        let native_site_call_dispatch = self.native_site_call_dispatch();

        Ok(quote! {
            mod native_abi_export {
                use super::#lib_ident;

                struct NativeLibInstance(Box<dyn ::sim::kernel::Lib>);

                #[allow(unsafe_code)]
                unsafe extern "C" fn instantiate() -> *mut ::std::ffi::c_void {
                    let instance = Box::new(NativeLibInstance(Box::new(#lib_ident)));
                    Box::into_raw(instance).cast::<::std::ffi::c_void>()
                }

                #[allow(unsafe_code)]
                unsafe extern "C" fn destroy_instance(instance: *mut ::std::ffi::c_void) {
                    if instance.is_null() {
                        return;
                    }
                    drop(Box::from_raw(instance.cast::<NativeLibInstance>()));
                }

                #[allow(unsafe_code)]
                unsafe extern "C" fn manifest(
                    instance: *mut ::std::ffi::c_void,
                ) -> ::sim::kernel::NativeAbiCallResponse {
                    let lib = &(*instance.cast::<NativeLibInstance>()).0;
                    match ::sim::loaders::encode_native_manifest_response(&lib.manifest()) {
                        Ok(response) => response,
                        Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                            ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                        ),
                    }
                }

                #[allow(unsafe_code)]
                unsafe extern "C" fn call(
                    instance: *mut ::std::ffi::c_void,
                    function: *const ::std::ffi::c_char,
                    args: ::sim::kernel::NativeAbiBorrowedBytes,
                ) -> ::sim::kernel::NativeAbiCallResponse {
                    let _lib = &(*instance.cast::<NativeLibInstance>()).0;
                    let function = if function.is_null() {
                        return ::sim::kernel::NativeAbiCallResponse::failure(
                            ::sim::kernel::NativeAbiError::boxed("native ABI call received a null function symbol"),
                        );
                    } else {
                        ::std::ffi::CStr::from_ptr(function).to_string_lossy().into_owned()
                    };
                    let arg_bytes = if args.ptr.is_null() && args.len == 0 {
                        &[][..]
                    } else {
                        ::std::slice::from_raw_parts(args.ptr, args.len)
                    };
                    let expr = match ::sim::codec_binary::decode_frame(::sim::kernel::CodecId(0), arg_bytes) {
                        Ok((_, expr)) => expr,
                        Err(err) => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            );
                        }
                    };
                    match native_class_call(function.as_str(), &expr) {
                        Ok(Some(expr)) => {
                            return match ::sim::codec_binary::encode_frame(&expr) {
                                Ok(frame) => ::sim::kernel::NativeAbiCallResponse::success(
                                    ::sim::kernel::native_abi_owned_bytes(frame.0),
                                ),
                                Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                                    ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                                ),
                            };
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            );
                        }
                    }
                    match native_macro_call(function.as_str(), &expr) {
                        Ok(Some(expr)) => {
                            return match ::sim::codec_binary::encode_frame(&expr) {
                                Ok(frame) => ::sim::kernel::NativeAbiCallResponse::success(
                                    ::sim::kernel::native_abi_owned_bytes(frame.0),
                                ),
                                Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                                    ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                                ),
                            };
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            );
                        }
                    }
                    match native_number_domain_call(function.as_str(), &expr) {
                        Ok(Some(expr)) => {
                            return match ::sim::codec_binary::encode_frame(&expr) {
                                Ok(frame) => ::sim::kernel::NativeAbiCallResponse::success(
                                    ::sim::kernel::native_abi_owned_bytes(frame.0),
                                ),
                                Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                                    ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                                ),
                            };
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            );
                        }
                    }
                    match native_codec_call(function.as_str(), &expr) {
                        Ok(Some(expr)) => {
                            return match ::sim::codec_binary::encode_frame(&expr) {
                                Ok(frame) => ::sim::kernel::NativeAbiCallResponse::success(
                                    ::sim::kernel::native_abi_owned_bytes(frame.0),
                                ),
                                Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                                    ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                                ),
                            };
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            );
                        }
                    }
                    match native_site_call(function.as_str(), &expr) {
                        Ok(Some(expr)) => {
                            return match ::sim::codec_binary::encode_frame(&expr) {
                                Ok(frame) => ::sim::kernel::NativeAbiCallResponse::success(
                                    ::sim::kernel::native_abi_owned_bytes(frame.0),
                                ),
                                Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                                    ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                                ),
                            };
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            );
                        }
                    }
                    let args = match expr {
                        ::sim::kernel::Expr::List(items) => items,
                        _ => {
                            return ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed("native ABI expected argument frame to decode to an expr list"),
                            );
                        }
                    };
                    match native_call(function.as_str(), args) {
                        Ok(expr) => match ::sim::codec_binary::encode_frame(&expr) {
                            Ok(frame) => ::sim::kernel::NativeAbiCallResponse::success(
                                ::sim::kernel::native_abi_owned_bytes(frame.0),
                            ),
                            Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                                ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                            ),
                        },
                        Err(err) => ::sim::kernel::NativeAbiCallResponse::failure(
                            ::sim::kernel::NativeAbiError::boxed(err.to_string()),
                        ),
                    }
                }

                #[allow(unsafe_code)]
                unsafe extern "C" fn destroy_bytes(bytes: ::sim::kernel::NativeAbiOwnedBytes) {
                    if !bytes.ptr.is_null() {
                        drop(Vec::from_raw_parts(bytes.ptr, bytes.len, bytes.cap));
                    }
                }

                #[allow(unsafe_code)]
                unsafe extern "C" fn destroy_error(error: *mut ::sim::kernel::NativeAbiError) {
                    if error.is_null() {
                        return;
                    }
                    let error = Box::from_raw(error);
                    if !error.message.is_null() {
                        drop(::std::ffi::CString::from_raw(error.message));
                    }
                }

                #native_class_call_dispatch
                #native_macro_call_dispatch
                #native_call_dispatch
                #native_codec_call_dispatch
                #native_number_domain_call_dispatch
                #native_site_call_dispatch

                static ABI: ::sim::kernel::NativeLibAbiV1 =
                    ::sim::kernel::NativeLibAbiV1::new(
                        instantiate,
                        destroy_instance,
                        manifest,
                        call,
                        destroy_bytes,
                        destroy_error,
                    );

                #[unsafe(no_mangle)]
                pub extern "C" fn sim_native_abi_v1() -> *const ::sim::kernel::NativeLibAbiV1 {
                    &ABI
                }
            }
        })
    }

    fn native_export_validation_error(&self) -> Option<String> {
        if let Some(message) = native_class_validation_error(&self.classes, &self.functions) {
            return Some(message);
        }
        for function in &self.functions {
            let ParsedFunctionKind::Function { .. } = function.kind else {
                continue;
            };
            for (_, ty) in &function.inputs {
                if matches!(ty, ParsedType::RefClass(_) | ParsedType::OwnedClass(_)) {
                    return Some(
                        "native_export = true only supports f64, bool, String, Symbol, and Expr arguments".to_owned(),
                    );
                }
            }
            if matches!(
                function.output,
                Some(ParsedType::RefClass(_)) | Some(ParsedType::OwnedClass(_))
            ) {
                return Some(
                    "native_export = true only supports f64, bool, String, Symbol, Expr, or nil returns".to_owned(),
                );
            }
        }
        None
    }

    fn native_codec_call_dispatch(&self) -> TokenStream2 {
        let arms = self.codecs.iter().flat_map(codec_dispatch_arms);
        let result_adapter = if self.codecs.is_empty() {
            TokenStream2::new()
        } else {
            quote! {
                trait NativeCodecResult<T> {
                    fn into_native_codec_result(self) -> ::sim::kernel::Result<T>;
                }

                impl NativeCodecResult<::sim::kernel::Expr> for ::sim::kernel::Expr {
                    fn into_native_codec_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                        Ok(self)
                    }
                }

                impl NativeCodecResult<::sim::kernel::Expr>
                    for ::sim::kernel::Result<::sim::kernel::Expr>
                {
                    fn into_native_codec_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                        self
                    }
                }

                impl NativeCodecResult<String> for String {
                    fn into_native_codec_result(self) -> ::sim::kernel::Result<String> {
                        Ok(self)
                    }
                }

                impl NativeCodecResult<String> for ::sim::kernel::Result<String> {
                    fn into_native_codec_result(self) -> ::sim::kernel::Result<String> {
                        self
                    }
                }
            }
        };

        quote! {
            #result_adapter

            fn native_codec_call(
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

    fn native_site_call_dispatch(&self) -> TokenStream2 {
        let arms = self.sites.iter().map(site_dispatch_arm);
        let result_adapter = if self.sites.is_empty() {
            TokenStream2::new()
        } else {
            quote! {
                trait NativeSiteResult {
                    fn into_native_site_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr>;
                }

                impl NativeSiteResult for ::sim::kernel::Expr {
                    fn into_native_site_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                        Ok(self)
                    }
                }

                impl NativeSiteResult for ::sim::kernel::Result<::sim::kernel::Expr> {
                    fn into_native_site_result(self) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                        self
                    }
                }
            }
        };

        quote! {
            #result_adapter

            fn native_site_call(
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

    fn native_call_dispatch(&self) -> TokenStream2 {
        let arms = self
            .functions
            .iter()
            .filter_map(|function| match function.kind {
                ParsedFunctionKind::Function { .. } => {
                    let rust_ident = &function.rust_ident;
                    let symbol_name = match &function.kind {
                        ParsedFunctionKind::Function { lisp_name } => lisp_name.clone(),
                        ParsedFunctionKind::Constructor { .. } => unreachable!(),
                    };
                    let symbol_lit = LitStr::new(&symbol_name, Span::call_site());
                    let expected_len = function.inputs.len();
                    let arg_bindings = function
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(index, (name, ty))| expr_binding_tokens(index, name, ty));
                    let call_args = function.inputs.iter().map(|(name, _)| quote!(#name));
                    let result_wrap =
                        expr_result_wrap_tokens(function.output.as_ref(), quote!(result));
                    Some(quote! {
                        #symbol_lit => {
                            if args.len() != #expected_len {
                                return Err(::sim::kernel::Error::Eval(format!(
                                    "{} expects {} args, got {}",
                                    #symbol_lit,
                                    #expected_len,
                                    args.len(),
                                )));
                            }
                            #(#arg_bindings)*
                            let result = super::#rust_ident(#(#call_args),*);
                            Ok(#result_wrap)
                        }
                    })
                }
                ParsedFunctionKind::Constructor { .. } => None,
            });

        quote! {
            fn native_call(
                function: &str,
                args: Vec<::sim::kernel::Expr>,
            ) -> ::sim::kernel::Result<::sim::kernel::Expr> {
                match function {
                    #(#arms,)*
                    _ => Err(::sim::kernel::Error::UnknownFunction {
                        function: ::sim::kernel::Symbol::new(function),
                    }),
                }
            }
        }
    }
}

fn site_dispatch_arm(site: &ParsedSite) -> TokenStream2 {
    let op = LitStr::new(&format!("{}/realize", site.symbol), Span::call_site());
    let realize_ident = &site.realize_ident;
    quote! {
        #op => {
            let args = match expr {
                ::sim::kernel::Expr::List(items) => items.clone(),
                other => {
                    return Err(::sim::kernel::Error::HostError(format!(
                        "{} expected realize args list, got {:?}",
                        #op,
                        other,
                    )));
                }
            };
            super::#realize_ident(args)
                .into_native_site_result()
                .map(Some)
        }
    }
}

fn codec_dispatch_arms(codec: &ParsedCodec) -> [TokenStream2; 2] {
    let decode_op = LitStr::new(&format!("{}/decode", codec.symbol), Span::call_site());
    let encode_op = LitStr::new(&format!("{}/encode", codec.symbol), Span::call_site());
    let decode_ident = &codec.decode_ident;
    let encode_ident = &codec.encode_ident;
    [
        quote! {
            #decode_op => {
                let text = match expr {
                    ::sim::kernel::Expr::String(text) => text.clone(),
                    other => {
                        return Err(::sim::kernel::Error::HostError(format!(
                            "{} expected decode input text, got {:?}",
                            #decode_op,
                            other,
                        )));
                    }
                };
                let decoded: ::sim::kernel::Expr =
                    NativeCodecResult::into_native_codec_result(super::#decode_ident(text))?;
                Ok(Some(decoded))
            }
        },
        quote! {
            #encode_op => {
                let rendered: String =
                    NativeCodecResult::into_native_codec_result(super::#encode_ident(expr.clone()))?;
                Ok(Some(::sim::kernel::Expr::String(rendered)))
            }
        },
    ]
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
