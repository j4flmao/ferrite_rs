use std::hash::{Hash, Hasher};

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, FnArg, ImplItem, ImplItemFn, ItemImpl, Pat, Token, Type};

pub fn expand(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item_impl: ItemImpl =
        syn::parse2(input).expect("`#[impl_controller]` can only be applied to an impl block");

    if let Some((_, path, _)) = &item_impl.trait_ {
        let msg = "`#[impl_controller]` does not support trait implementations";
        return syn::Error::new_spanned(path, msg).to_compile_error();
    }

    let controller_ty = match &*item_impl.self_ty {
        Type::Path(p) if p.qself.is_none() => p.path.clone(),
        other => {
            let msg = "`#[impl_controller]` requires a concrete self type";
            return syn::Error::new_spanned(other, msg).to_compile_error();
        }
    };

    // Controller-scoped pipeline (instead of per-method, Nest's `@UseGuards`
    // can also live on the class; here it lives on the impl block).
    let impl_pipeline = take_pipeline(&mut item_impl.attrs);

    let mut routes = Vec::new();

    for item in item_impl.items.iter_mut() {
        let im_fn = match item {
            ImplItem::Fn(f) => f,
            _ => continue,
        };

        let Some(method_attr) = take_route_attr(&mut im_fn.attrs) else {
            continue;
        };
        let method_pipeline = take_pipeline(&mut im_fn.attrs);

        match build_route(
            &controller_ty,
            im_fn,
            &method_attr,
            &impl_pipeline,
            &method_pipeline,
        ) {
            Ok(route) => routes.push(route),
            Err(err) => return err,
        }
    }

    if routes.is_empty() {
        let msg = "`#[impl_controller]` must contain at least one `#[get]/#[post]/...` method";
        return syn::Error::new_spanned(&item_impl, msg).to_compile_error();
    }

    let mut route_fns = TokenStream::new();
    let mut route_specs = Vec::new();

    for route in routes {
        let Route {
            method,
            path,
            build_ident,
            build_fn,
            request_body_ty,
            guard_types,
        } = route;
        route_fns.extend(build_fn);
        route_specs.push(quote! {
            ::ferrite_framework::_http::RouteSpec {
                method: #method,
                path: #path,
                build: #build_ident,
                request_body_types: #request_body_ty,
                guard_types: #guard_types,
            }
        });
    }

    quote! {
        #item_impl

        #route_fns

        ::ferrite_framework::__export::inventory::submit! {
            ::ferrite_framework::_http::ControllerRoutes {
                controller: ::std::any::TypeId::of::<#controller_ty>(),
                prefix: <#controller_ty>::__ferrite_prefix(),
                routes: &[#(#route_specs),*],
            }
        }
    }
}

/// Guards / interceptors / middleware / filters collected at a scope.
#[derive(Default, Clone)]
struct PipelineTypes {
    guards: Vec<syn::Type>,
    interceptors: Vec<syn::Type>,
    middleware: Vec<syn::Type>,
    filters: Vec<syn::Type>,
}

impl PipelineTypes {
    fn merge(&mut self, other: &PipelineTypes) {
        self.guards.extend(other.guards.iter().cloned());
        self.interceptors.extend(other.interceptors.iter().cloned());
        self.middleware.extend(other.middleware.iter().cloned());
        self.filters.extend(other.filters.iter().cloned());
    }

    fn combined(&self, other: &PipelineTypes) -> PipelineTypes {
        let mut out = self.clone();
        out.merge(other);
        out
    }

    fn is_empty(&self) -> bool {
        self.guards.is_empty()
            && self.interceptors.is_empty()
            && self.middleware.is_empty()
            && self.filters.is_empty()
    }
}

fn take_pipeline(attrs: &mut Vec<Attribute>) -> PipelineTypes {
    let mut out = PipelineTypes::default();
    attrs.retain(|attr| {
        let name = attr.path().segments.last().unwrap().ident.to_string();
        let list = match &attr.meta {
            syn::Meta::List(list) => list,
            _ => return true,
        };

        let types = Punctuated::<syn::Type, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .ok()
            .map(|p| p.into_iter().collect::<Vec<_>>());

        let Some(types) = types else { return true };

        match name.as_str() {
            "use_guards" => out.guards.extend(types),
            "use_interceptors" => out.interceptors.extend(types),
            "use_middleware" => out.middleware.extend(types),
            "catch" => out.filters.extend(types),
            _ => return true,
        }
        false
    });
    out
}

fn build_route(
    controller_ty: &syn::Path,
    im_fn: &ImplItemFn,
    attr: &RouteAttr,
    impl_pipeline: &PipelineTypes,
    method_pipeline: &PipelineTypes,
) -> Result<Route, TokenStream> {
    let recv_ok =
        matches!(im_fn.sig.inputs.first(), Some(FnArg::Receiver(r)) if r.reference.is_some());
    if !recv_ok {
        let msg = "route handlers must take `&self`";
        return Err(syn::Error::new_spanned(&im_fn.sig.ident, msg).to_compile_error());
    }
    if im_fn.sig.asyncness.is_none() {
        let msg = "route handlers must be `async fn`";
        return Err(syn::Error::new_spanned(&im_fn.sig.ident, msg).to_compile_error());
    }

    let mut closure_params = Vec::new();
    let mut call_args = Vec::new();
    let mut request_body_types: Vec<String> = Vec::new();

    for input in im_fn.sig.inputs.iter().skip(1) {
        let syn::FnArg::Typed(pat_type) = input else {
            continue;
        };

        let pat = &pat_type.pat;
        let ty = &pat_type.ty;
        let arg = pat_to_expr(pat);
        let pat_tokens = pat.to_token_stream();
        let ty_tokens = ty.to_token_stream();
        closure_params.push(quote! { #pat_tokens: #ty_tokens });
        call_args.push(quote! { #arg });

        // Heuristic: any `Xyz(inner: Dto)` pattern (extracts like `Json`,
        // `Form`, `ValidJson`, `ValidForm`) whose inner generic arg is a
        // user struct → emit its fully qualified name for OpenAPI requestBody.
        if let syn::Type::Path(tp) = &**ty {
            let last = tp.path.segments.last();
            if let Some(seg) = last {
                let seg_name = seg.ident.to_string();
                let is_json_extractor = matches!(
                    seg_name.as_str(),
                    "Json" | "ValidJson" | "Form" | "ValidForm"
                );
                if is_json_extractor {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        for a in args.args.iter() {
                            if let syn::GenericArgument::Type(inner_ty) = a {
                                request_body_types.push(type_fq_name(inner_ty));
                            }
                        }
                    }
                }
            }
        }
    }

    let request_body_ty = if request_body_types.is_empty() {
        String::new()
    } else {
        request_body_types.join(",")
    };

    let method_ident = &im_fn.sig.ident;
    let ctr_name = controller_ty.segments.last().unwrap().ident.to_string();
    let id = hash(&format!("{ctr_name}-{}-{method_ident}", attr.path));
    let build_ident = format_ident!("__ferrite_route_{id}");
    let routing = &attr.routing_fn;
    let path = &attr.path;

    let closure = if closure_params.is_empty() {
        quote! { move || async move { ctl.#method_ident().await } }
    } else {
        quote! { move |#(#closure_params),*| async move {
            ctl.#method_ident(#(#call_args),*).await
        } }
    };

    let pipeline = impl_pipeline.combined(method_pipeline);

    let wired = wire_pipeline(&pipeline);

    let need_layer = !pipeline.is_empty();

    let build_fn = if need_layer {
        quote! {
            #[allow(non_snake_case)]
            fn #build_ident(container: &::ferrite_framework::Container) -> ::ferrite_framework::_http::axum::Router {
                let ctl: ::std::sync::Arc<#controller_ty> = container.get::<#controller_ty>();
                #wired
                let router = ::ferrite_framework::_http::axum::Router::new().route(
                    #path,
                    ::ferrite_framework::_http::axum::routing::#routing(#closure),
                );
                router.route_layer(
                    ::ferrite_framework::_http::axum::middleware::from_fn_with_state(
                        pipeline,
                        ::ferrite_framework::_http::RoutePipeline::run,
                    ),
                )
            }
        }
    } else {
        quote! {
            #[allow(non_snake_case)]
            fn #build_ident(container: &::ferrite_framework::Container) -> ::ferrite_framework::_http::axum::Router {
                let ctl: ::std::sync::Arc<#controller_ty> = container.get::<#controller_ty>();
                ::ferrite_framework::_http::axum::Router::new().route(
                    #path,
                    ::ferrite_framework::_http::axum::routing::#routing(#closure),
                )
            }
        }
    };

    Ok(Route {
        method: attr.method,
        path: attr.path.clone(),
        build_ident,
        build_fn,
        request_body_ty,
        guard_types: pipeline
            .guards
            .iter()
            .map(type_fq_name)
            .collect::<Vec<_>>()
            .join(","),
    })
}

fn type_fq_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .iter()
            .map(|s| {
                let mut s_name = s.ident.to_string();
                if let syn::PathArguments::AngleBracketed(args) = &s.arguments {
                    let inner: Vec<String> = args
                        .args
                        .iter()
                        .filter_map(|a| match a {
                            syn::GenericArgument::Type(t) => Some(type_fq_name(t)),
                            syn::GenericArgument::Lifetime(_)
                            | syn::GenericArgument::Const(_)
                            | syn::GenericArgument::AssocType(_)
                            | syn::GenericArgument::AssocConst(_)
                            | syn::GenericArgument::Constraint(_) => None,
                            _ => None,
                        })
                        .collect();
                    if !inner.is_empty() {
                        s_name.push(';');
                        s_name.push_str(&inner.join(";"));
                    }
                }
                s_name
            })
            .collect::<Vec<_>>()
            .join("::"),
        syn::Type::Reference(r) => type_fq_name(&r.elem),
        syn::Type::Slice(s) => format!("[{}]", type_fq_name(&s.elem)),
        syn::Type::Tuple(t) => format!(
            "({})",
            t.elems
                .iter()
                .map(type_fq_name)
                .collect::<Vec<_>>()
                .join(",")
        ),
        syn::Type::Paren(p) => type_fq_name(&p.elem),
        syn::Type::Array(a) => format!("[{}]", type_fq_name(&a.elem)),
        other => format!("{:?}", other),
    }
}

/// Emit the code that builds the `RoutePipeline` from the DI container.
/// Returns the `pipeline` binding; marks it `mut` only when something is added.
fn wire_pipeline(p: &PipelineTypes) -> TokenStream {
    if p.is_empty() {
        return quote! { let mut pipeline = ::ferrite_framework::_http::RoutePipeline::new(); };
    }

    let mut stmts = TokenStream::new();
    stmts.extend(quote! { let mut pipeline = ::ferrite_framework::_http::RoutePipeline::new(); });

    for g in &p.guards {
        let ty = g.to_token_stream();
        stmts.extend(quote! {
            let _guard: ::std::sync::Arc<dyn ::ferrite_framework::_http::Guard> = container.get::<#ty>();
            pipeline = pipeline.guard(_guard);
        });
    }
    for i in &p.interceptors {
        let ty = i.to_token_stream();
        stmts.extend(quote! {
            let _interceptor: ::std::sync::Arc<dyn ::ferrite_framework::_http::Interceptor> = container.get::<#ty>();
            pipeline = pipeline.interceptor(_interceptor);
        });
    }
    for m in &p.middleware {
        let ty = m.to_token_stream();
        stmts.extend(quote! {
            let _middleware: ::std::sync::Arc<dyn ::ferrite_framework::_http::Middleware> = container.get::<#ty>();
            pipeline = pipeline.middleware(_middleware);
        });
    }
    for f in &p.filters {
        let ty = f.to_token_stream();
        stmts.extend(quote! {
            let _filter: ::std::sync::Arc<dyn ::ferrite_framework::_http::ExceptionFilter> = container.get::<#ty>();
            pipeline = pipeline.filter(_filter);
        });
    }

    stmts
}

struct Route {
    method: &'static str,
    path: String,
    build_ident: proc_macro2::Ident,
    build_fn: TokenStream,
    request_body_ty: String,
    guard_types: String,
}

struct RouteAttr {
    method: &'static str,
    routing_fn: proc_macro2::Ident,
    path: String,
}

fn take_route_attr(attrs: &mut Vec<Attribute>) -> Option<RouteAttr> {
    let mut out = None;
    attrs.retain(|attr| {
        let ident = attr.path().segments.last().unwrap().ident.to_string();
        let (method, routing) = match ident.as_str() {
            "get" => ("GET", "get"),
            "post" => ("POST", "post"),
            "put" => ("PUT", "put"),
            "patch" => ("PATCH", "patch"),
            "delete" => ("DELETE", "delete"),
            "all" => ("ALL", "any"),
            _ => return true,
        };

        let path = read_attr_path(attr);
        match path {
            Some(p) => {
                out = Some(RouteAttr {
                    method,
                    routing_fn: format_ident!("{routing}"),
                    path: p,
                });
                false
            }
            None => true,
        }
    });

    out
}

fn read_attr_path(attr: &Attribute) -> Option<String> {
    let syn::Meta::List(list) = &attr.meta else {
        return None;
    };
    for part in list.tokens.to_string().split(',') {
        let t = part.trim().trim_matches('"');
        if !t.is_empty() && !t.starts_with('{') && !t.contains('(') && !t.contains('=') {
            return Some(t.to_string());
        }
    }
    None
}

fn pat_to_expr(pat: &Pat) -> TokenStream {
    let toks = pat.to_token_stream();
    let s = toks.to_string();
    if let Some(rest) = s.strip_prefix("mut ") {
        return rest.parse().unwrap_or(toks);
    }
    toks
}

fn hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
