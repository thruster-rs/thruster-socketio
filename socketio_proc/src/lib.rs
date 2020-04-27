extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span as Span2};
use quote::quote;

#[proc_macro_attribute]
pub fn socketio_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let syn::Item::Fn(mut function_item) = syn::parse(item.clone()).unwrap() {
        let name = function_item.ident.clone();
        let new_name = Ident::new(&format!("__async_{}", name.clone()), Span2::call_site());
        function_item.ident = new_name.clone();

        let visibility = function_item.vis.clone();
        let arguments = function_item.decl.inputs.clone();
        let generics = function_item.decl.generics.clone();

        let context_type = match &arguments[0] {
            syn::FnArg::Captured(cap) => &cap.ty,
            _ => panic!("Expected the first argument to be a context type"),
        };
        let socket_io_type = Ident::new(&format!("__SocketIO_{}", name.clone()), Span2::call_site());
        let crate_path = match attr.to_string().as_str() {
            "_internal" => quote! {
                crate::core::{ IOSocketWrapper as #socket_io_type }
            },
            _ => quote! {
                thruster_socketio::{ SocketIO as #socket_io_type }
            },
        };

        let gen = quote! {
            #function_item

            use #crate_path;
            #visibility fn #name#generics(socket: #context_type) -> Pin<Box<dyn Future<Output = Result<#context_type, ()>> + Send>> {
                Box::pin(#new_name(socket))
            }
        };

        // proc_macro::Span::call_site()
        //     .note("Thruster code output")
        //     .note(gen.to_string())
        //     .emit();

        gen.into()
    } else {
        item
    }
}

#[proc_macro_attribute]
pub fn socketio_listener(attr: TokenStream, item: TokenStream) -> TokenStream {
    if let syn::Item::Fn(mut function_item) = syn::parse(item.clone()).unwrap() {
        let name = function_item.ident.clone();
        let new_name = Ident::new(&format!("__async_{}", name.clone()), Span2::call_site());
        function_item.ident = new_name.clone();

        let visibility = function_item.vis.clone();
        let arguments = function_item.decl.inputs.clone();
        let generics = function_item.decl.generics.clone();

        let socket_type = match &arguments[0] {
            syn::FnArg::Captured(cap) => &cap.ty,
            _ => panic!("Expected the first argument to be a socket type"),
        };

        let socket_io_type = Ident::new(&format!("__SocketIOListener_{}", name.clone()), Span2::call_site());
        let crate_path = match attr.to_string().as_str() {
            "_internal" => quote! {
                crate::core::{ IOSocketWrapper as #socket_io_type }
            },
            _ => quote! {
                thruster_socketio::{ SocketIO as #socket_io_type }
            },
        };

        let gen = quote! {
            #function_item

            use #crate_path;
            #visibility fn #name#generics(socket: #socket_type, value: String) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send>> {
                Box::pin(#new_name(socket, value))
            }
        };

        // proc_macro::Span::call_site()
        //     .note("Thruster code output")
        //     .note(gen.to_string())
        //     .emit();

        gen.into()
    } else {
        item
    }
}
