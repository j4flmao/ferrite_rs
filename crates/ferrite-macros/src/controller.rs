use proc_macro2::TokenStream;
use quote::quote;
use syn::{punctuated::Punctuated, ExprAssign, ItemStruct, Token};

pub fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut struct_ast: ItemStruct =
        syn::parse2(input.clone()).expect("`#[controller]` can only be applied to a struct");

    if let Some(gen) = struct_ast.generics.params.first() {
        let msg = "`#[controller]` does not support generic types";
        return syn::Error::new_spanned(gen, msg).to_compile_error();
    }

    if matches!(struct_ast.fields, syn::Fields::Unit) {
        let msg = "`#[controller]` requires at least one field";
        return syn::Error::new_spanned(&struct_ast.ident, msg).to_compile_error();
    }

    let prefix = parse_prefix(&args);
    let name = struct_ast.ident.clone();

    rewrite_fields(&mut struct_ast);

    quote! {
        #struct_ast

        impl #name {
            /// Route prefix declared on `#[controller(...)]`.
            pub const fn __ferrite_prefix() -> &'static str { #prefix }
        }

        impl ::ferrite_framework::Injectable for #name {
            fn __provider_entry() -> ::ferrite_framework::ProviderEntry {
                ::ferrite_framework::ProviderEntry::new_static::<#name>(
                    stringify!(#name),
                    ::ferrite_framework::Scope::Singleton,
                    <#name>::__ferrite_deps,
                    <#name>::__ferrite_inject,
                )
            }
        }

        ::ferrite_framework::__export::inventory::submit! {
            ::ferrite_framework::ProviderEntry::new_static::<#name>(
                stringify!(#name),
                ::ferrite_framework::Scope::Singleton,
                <#name>::__ferrite_deps,
                <#name>::__ferrite_inject,
            )
        }
    }
}

fn parse_prefix(args: &TokenStream) -> String {
    let s = args.to_string().trim().to_string();
    let s = s
        .trim_matches(|c| c == '"' || c == ' ' || c == ']' || c == '[')
        .to_string();
    if s.is_empty() || s == "version" || s.contains('=') {
        // e.g. `#[controller("/users", version = "v1")]` — take first quoted arg.
        if let Some(start) = s.find('"') {
            if let Some(end_rel) = s[start + 1..].find('"') {
                return s[start + 1..start + 1 + end_rel].to_string();
            }
        }
        return "/".to_string();
    }
    s
}

fn rewrite_fields(struct_ast: &mut ItemStruct) {
    match &mut struct_ast.fields {
        syn::Fields::Named(named) => {
            for field in named.named.iter_mut() {
                let ty = field.ty.clone();
                field.ty = syn::parse_quote! { ::std::sync::Arc<#ty> };
            }
        }
        syn::Fields::Unnamed(unnamed) => {
            for field in unnamed.unnamed.iter_mut() {
                let ty = field.ty.clone();
                field.ty = syn::parse_quote! { ::std::sync::Arc<#ty> };
            }
        }
        syn::Fields::Unit => {}
    }
}

fn _noop(_: Punctuated<ExprAssign, Token![,]>) {}
