use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

pub fn expand(input: TokenStream) -> TokenStream {
    let mut item_fn: syn::ItemFn =
        syn::parse2(input).expect("`#[inject]` can only be applied to a function");

    if item_fn.sig.asyncness.is_some() {
        let msg = "`#[inject]` constructors must be synchronous";
        return syn::Error::new_spanned(&item_fn.sig, msg).to_compile_error();
    }

    let ctor_ident = &item_fn.sig.ident;

    let dep_types: Vec<TokenStream> = item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(pattern) => {
                let ty = pattern.ty.to_token_stream();
                Some(quote! { #ty })
            }
            syn::FnArg::Receiver(_) => None,
        })
        .collect();

    // Rewrite each parameter type to Arc<T>.
    for input in item_fn.sig.inputs.iter_mut() {
        if let syn::FnArg::Typed(pattern) = input {
            let ty = pattern.ty.clone();
            pattern.ty = syn::parse_quote! { ::std::sync::Arc<#ty> };
        }
    }

    let deps_terms: Vec<TokenStream> = dep_types
        .iter()
        .map(|t| quote! { ::std::any::TypeId::of::<#t>() })
        .collect();

    let inject_body = if dep_types.is_empty() {
        quote! {
            let inner = Self::#ctor_ident();
            let arc: ::std::sync::Arc<Self> = ::std::sync::Arc::new(inner);
            let any: ::ferrite_framework::AnyArc = arc;
            any
        }
    } else {
        let gets = dep_types
            .iter()
            .map(|t| quote! { container.get::<#t>() })
            .collect::<Vec<_>>();
        quote! {
            let inner = Self::#ctor_ident(#(#gets),*);
            let arc: ::std::sync::Arc<Self> = ::std::sync::Arc::new(inner);
            let any: ::ferrite_framework::AnyArc = arc;
            any
        }
    };

    quote! {
        #item_fn

        /// Resolve a fully-constructed instance from the DI container.
        pub fn __ferrite_inject(container: &::ferrite_framework::Container) -> ::ferrite_framework::AnyArc {
            #inject_body
        }

        /// Declare this provider's dependencies (for the DI graph).
        pub fn __ferrite_deps() -> ::std::vec::Vec<::std::any::TypeId> {
            ::std::vec![#(#deps_terms),*]
        }
    }
}
