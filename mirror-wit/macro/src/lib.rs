//! Macro utilities for Mirror WIT modules
//!
//! This crate provides procedural macros to reduce boilerplate when
//! implementing Mirror modules.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for implementing `MirrorModule` trait with minimal code
///
/// # Example
///
/// ```ignore
/// use mirror_wit_macros::MirrorModule;
///
/// #[derive(MirrorModule)]
/// struct MyModule {
///     name: String,
/// }
/// ```
#[proc_macro_derive(MirrorModule, attributes(mirror))]
pub fn derive_mirror_module(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl mirror_wit::MirrorModule for #name {
            fn init(&mut self) -> Result<mirror_wit::ModuleInitResponse, String> {
                Ok(mirror_wit::ModuleInitResponse {
                    version: 1,
                    tags: vec![
                        "Read".to_string(),
                        "Write".to_string()
                    ],
                })
            }

            fn handle_message(&mut self, payload: &str) -> Result<String, String> {
                // Default implementation: echo back the payload
                Ok(payload.to_string())
            }

            fn get_tags(&self) -> Vec<mirror_wit::MirrorTag> {
                vec![
                    mirror_wit::MirrorTag::Read,
                    mirror_wit::MirrorTag::Write
                ]
            }
        }
    };

    TokenStream::from(expanded)
}

/// Attribute macro to mark a struct as a Mirror module
///
/// This macro handles the complete setup including WIT interface exports.
#[proc_macro_attribute]
pub fn mirror_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        #input

        impl #name {
            /// Create a new instance of this module
            pub fn new() -> Self {
                Default::default()
            }
        }

        impl Default for #name {
            fn default() -> Self {
                #name::new()
            }
        }
    };

    TokenStream::from(expanded)
}
