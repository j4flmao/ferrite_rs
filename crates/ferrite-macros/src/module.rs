use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprArray, ExprAssign, ItemStruct, Token};

pub fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let struct_ast: ItemStruct =
        syn::parse2(input.clone()).expect("`#[module]` can only be applied to a struct");

    if let Some(gen) = struct_ast.generics.params.first() {
        let msg = "`#[module]` does not support generic modules";
        return syn::Error::new_spanned(gen, msg).to_compile_error();
    }

    let opts = ModuleOptions::parse(&args);
    let name = &struct_ast.ident;
    let mod_name_str = name.to_string();

    let imports = type_id_terms(&opts.imports);
    let controllers = type_id_terms(&opts.controllers);
    let providers = type_id_terms(&opts.providers);
    let exports = type_id_terms(&opts.exports);
    let global = opts.global;

    // Compile-time trait-bound assertions. Instead of runtime panics such as
    // "provider TypeId(...) is not registered", emit a dummy function that
    // dereferences the trait's associated item — the compiler will point at
    // the exact attribute span when a type is missing the required bound.
    let assert_imports = opts.imports.iter().map(|p| {
        quote! {
            #[allow(dead_code, unused)]
            const _: () = {
                fn _assert_module<T: ::ferrite_framework::Module>() {}
                #[allow(non_upper_case_globals)]
                const _X: fn() = _assert_module::<#p>;
            };
        }
    });
    let assert_providers = opts.providers.iter().map(|p| {
        quote! {
            #[allow(dead_code, unused)]
            const _: () = {
                fn _assert_injectable<T: ::ferrite_framework::Injectable>() {}
                #[allow(non_upper_case_globals)]
                const _X: fn() = _assert_injectable::<#p>;
            };
        }
    });
    let assert_controllers = opts.controllers.iter().map(|p| {
        quote! {
            #[allow(dead_code, unused)]
            const _: () = {
                fn _assert_injectable<T: ::ferrite_framework::Injectable>() {}
                #[allow(non_upper_case_globals)]
                const _X: fn() = _assert_injectable::<#p>;
            };
        }
    });
    let assert_exports = opts.exports.iter().map(|p| {
        quote! {
            #[allow(dead_code, unused)]
            const _: () = {
                fn _assert_injectable<T: ::ferrite_framework::Injectable>() {}
                #[allow(non_upper_case_globals)]
                const _X: fn() = _assert_injectable::<#p>;
            };
        }
    });

    quote! {
        #struct_ast

        #(#assert_imports)*
        #(#assert_providers)*
        #(#assert_controllers)*
        #(#assert_exports)*

        impl ::ferrite_framework::Module for #name {
            fn __module_descriptor() -> ::ferrite_framework::ModuleDescriptor {
                ::ferrite_framework::ModuleDescriptor {
                    name: String::from(#mod_name_str),
                    imports: vec![#(#imports),*],
                    controllers: vec![#(#controllers),*],
                    providers: vec![#(#providers),*],
                    exports: vec![#(#exports),*],
                    middleware: vec![],
                    global: #global,
                }
            }
        }

        ::ferrite_framework::__export::inventory::submit! {
            ::ferrite_framework::ModuleEntry::new::<#name>(stringify!(#name))
        }
    }
}

pub struct ModuleOptions {
    pub imports: Vec<syn::Path>,
    pub controllers: Vec<syn::Path>,
    pub providers: Vec<syn::Path>,
    pub exports: Vec<syn::Path>,
    pub global: bool,
}

impl ModuleOptions {
    fn parse(args: &TokenStream) -> Self {
        let mut out = Self {
            imports: vec![],
            controllers: vec![],
            providers: vec![],
            exports: vec![],
            global: false,
        };

        if args.is_empty() {
            return out;
        }

        for assign in Punctuated::<ExprAssign, Token![,]>::parse_terminated
            .parse2(args.clone())
            .expect("invalid `#[module(...)]` arguments")
        {
            let key = expr_key(&assign.left);
            match key.as_str() {
                "imports" => out.imports = expr_path_list(&assign.right),
                "controllers" => out.controllers = expr_path_list(&assign.right),
                "providers" => out.providers = expr_path_list(&assign.right),
                "exports" => out.exports = expr_path_list(&assign.right),
                "global" => out.global = true,
                _ => {}
            }
        }

        let raw = args.to_string();
        if raw.contains("global") {
            out.global = true;
        }

        out
    }
}

fn expr_key(expr: &Expr) -> String {
    match expr {
        Expr::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn expr_path_list(expr: &Expr) -> Vec<syn::Path> {
    match expr {
        Expr::Array(ExprArray { elems, .. }) => elems
            .iter()
            .filter_map(|e| match e {
                Expr::Path(p) => Some(p.path.clone()),
                _ => None,
            })
            .collect(),
        Expr::Path(p) => vec![p.path.clone()],
        _ => vec![],
    }
}

fn type_id_terms(paths: &[syn::Path]) -> Vec<TokenStream> {
    paths
        .iter()
        .map(|p| {
            let t = p.to_token_stream();
            quote! { ::std::any::TypeId::of::<#t>() }
        })
        .collect()
}
