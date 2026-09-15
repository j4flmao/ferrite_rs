use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Fields, Ident, ItemStruct, Lit, Meta, Token};

/// Expand `#[entity(...)]` into the `Entity` impl plus a `Repository<T>`
/// provider registration.
///
/// Accepted arguments (all optional):
/// - `table = "users"` — table name (default: the struct's name).
/// - `primary_key = "id"` — name of the primary key field (default `id`).
/// - `store = "path::to::Pick"` — picker used to build the store backend;
///   defaults to the in-memory picker. The path is re-emitted verbatim.
pub fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item: ItemStruct =
        syn::parse2(input.clone()).expect("`#[entity]` can only be applied to a struct");

    if let Some(gen) = item.generics.params.first() {
        let msg = "`#[entity]` does not support generic types";
        return syn::Error::new_spanned(gen, msg).to_compile_error();
    }

    let name = item.ident.clone();

    let table = match find_str_attr(&args, "table") {
        Some(v) => v,
        None => name.to_string(),
    };

    let pk_field = match find_str_attr(&args, "primary_key") {
        Some(v) => v,
        None => "id".to_string(),
    };
    let pk_ident = Ident::new(&pk_field, name.span());
    let pk_ty = match &mut item.fields {
        Fields::Named(named) => {
            match named
                .named
                .iter_mut()
                .find(|f| f.ident.as_ref() == Some(&pk_ident))
            {
                Some(f) => f.ty.clone(),
                None => {
                    let msg =
                        format!("`#[entity]` primary key field `{pk_field}` not found on `{name}`");
                    return syn::Error::new_spanned(&name, msg).to_compile_error();
                }
            }
        }
        _ => {
            let msg = "`#[entity]` requires a named-field struct";
            return syn::Error::new_spanned(&name, msg).to_compile_error();
        }
    };

    let store = match find_str_attr(&args, "store") {
        Some(p) => match syn::parse_str::<syn::Path>(&p) {
            Ok(path) => path,
            Err(err) => return syn::Error::new(name.span(), err).to_compile_error(),
        },
        None => syn::parse_quote!(::ferrite_framework::orm::InMemoryPick),
    };

    let deps_fn = format_ident!("__ferrite_repo_deps_{}", name);
    let factory_fn = format_ident!("__ferrite_repo_factory_{}", name);

    quote! {
        #item

        impl ::ferrite_framework::orm::Entity for #name {
            type PrimaryKey = #pk_ty;

            fn table_name() -> &'static str {
                #table
            }

            fn primary_key(&self) -> Self::PrimaryKey {
                self.#pk_ident.clone()
            }
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #deps_fn() -> ::std::vec::Vec<::std::any::TypeId> {
            ::std::vec![::std::any::TypeId::of::<#store>()]
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #factory_fn(c: &::ferrite_framework::Container) -> ::ferrite_framework::AnyArc {
            let pick: ::std::sync::Arc<#store> = c.get::<#store>();
            let store = ::ferrite_framework::orm::Pick::<#name>::build(pick.as_ref());
            ::std::sync::Arc::new(::ferrite_framework::orm::Repository::<#name>::new(store))
                as ::ferrite_framework::AnyArc
        }

        ::ferrite_framework::__export::inventory::submit! {
            ::ferrite_framework::ProviderEntry::new_static::<::ferrite_framework::orm::Repository<#name>>(
                ::core::concat!("Repository<", ::core::stringify!(#name), ">"),
                ::ferrite_framework::Scope::Singleton,
                #deps_fn,
                #factory_fn,
            )
        }
    }
}

fn find_str_attr(args: &TokenStream, key: &str) -> Option<String> {
    for meta in Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(args.clone())
        .unwrap_or_default()
    {
        if let Meta::NameValue(nv) = meta {
            let k = nv.path.segments.last().unwrap().ident.to_string();
            if k == key {
                if let syn::Expr::Lit(el) = &nv.value {
                    if let Lit::Str(s) = &el.lit {
                        return Some(s.value());
                    }
                }
            }
        }
    }
    None
}
