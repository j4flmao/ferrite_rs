use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{ExprAssign, ItemStruct, Token};

pub fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut struct_ast: ItemStruct =
        syn::parse2(input.clone()).expect("`#[injectable]` can only be applied to a struct");

    if let Some(gen) = struct_ast.generics.params.first() {
        let msg = "`#[injectable]` does not support generic types";
        return syn::Error::new_spanned(gen, msg).to_compile_error();
    }

    if matches!(struct_ast.fields, syn::Fields::Unit) {
        let msg = "`#[injectable]` requires at least one field";
        return syn::Error::new_spanned(&struct_ast.ident, msg).to_compile_error();
    }

    let scope_expr = parse_scope(&args);
    let name = struct_ast.ident.clone();

    // Wrap every field type in Arc<T> (Deref keeps usage ergonomic).
    rewrite_fields(&mut struct_ast);

    quote! {
        #struct_ast

        impl ::ferrite_framework::Injectable for #name {
            fn __provider_entry() -> ::ferrite_framework::ProviderEntry {
                ::ferrite_framework::ProviderEntry::new_static::<#name>(
                    stringify!(#name),
                    #scope_expr,
                    <#name>::__ferrite_deps,
                    <#name>::__ferrite_inject,
                )
            }
        }

        ::ferrite_framework::__export::inventory::submit! {
            ::ferrite_framework::ProviderEntry::new_static::<#name>(
                stringify!(#name),
                #scope_expr,
                <#name>::__ferrite_deps,
                <#name>::__ferrite_inject,
            )
        }
    }
}

fn parse_scope(args: &TokenStream) -> TokenStream {
    // Looks for `scope = "request"` etc.
    for assign in Punctuated::<ExprAssign, Token![,]>::parse_terminated
        .parse2(args.clone())
        .unwrap_or_default()
    {
        let key = match &*assign.left {
            syn::Expr::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
            _ => continue,
        };
        if key == "scope" {
            if let syn::Expr::Lit(l) = &*assign.right {
                if let syn::Lit::Str(s) = &l.lit {
                    let v = s.value().to_ascii_lowercase();
                    let scope = match v.as_str() {
                        "request" => ::syn::parse_quote!(::ferrite_framework::Scope::Request),
                        "transient" => ::syn::parse_quote!(::ferrite_framework::Scope::Transient),
                        _ => ::syn::parse_quote!(::ferrite_framework::Scope::Singleton),
                    };
                    return scope;
                }
            }
        }
    }
    let _ = |_x: &syn::Meta| {};
    ::syn::parse_quote!(::ferrite_framework::Scope::Singleton)
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
