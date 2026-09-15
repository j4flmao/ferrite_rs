use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::{Attribute, DeriveInput, Expr, Field, Lit, Meta, Token};

/// A single `#[validate(...)]` rule attached to a field.
enum Rule {
    Email,
    NotEmpty,
    Length {
        min: Option<usize>,
        max: Option<usize>,
    },
    Range {
        min: Option<f64>,
        max: Option<f64>,
    },
}

pub fn expand(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse2(input).expect("`#[derive(Validate)]` needs a struct");

    if !ast.generics.params.is_empty() {
        let msg = "`#[derive(Validate)]` does not support generic types";
        return syn::Error::new_spanned(&ast.generics, msg).to_compile_error();
    }

    let struct_ident = &ast.ident;

    let syn::Data::Struct(data) = &ast.data else {
        let msg = "`#[derive(Validate)]` can only be applied to a struct";
        return syn::Error::new_spanned(&ast.ident, msg).to_compile_error();
    };

    let checks = build_checks(&data.fields);

    quote! {
        impl ::ferrite_framework::validation::Validate for #struct_ident {
            fn validate(&self) -> ::std::result::Result<(), ::ferrite_framework::validation::ValidationErrors> {
                let mut errors = ::ferrite_framework::validation::ValidationErrors::new();
                #(#checks)*
                if errors.is_empty() {
                    ::std::result::Result::Ok(())
                } else {
                    ::std::result::Result::Err(errors)
                }
            }
        }
    }
}

fn build_checks(fields: &syn::Fields) -> Vec<TokenStream> {
    fields
        .iter()
        .filter_map(|field| {
            let rules = match parse_rules(field) {
                Ok(rules) => rules,
                Err(err) => return Some(err.to_compile_error()),
            };
            if rules.is_empty() {
                return None;
            }

            let ident = field.ident.clone()?;
            let field_name = ident.to_string();

            // Reject rules on collection/optional fields with a clear message.
            let kind = field.ty.to_token_stream().to_string();
            if kind.starts_with("Option") || kind.starts_with("Vec") {
                return Some(compile_error(field, &format!(
                    "`{field_name}`: #[validate(...)] is not supported on Option/Vec fields"
                )));
            }

            let mut checks = Vec::new();
            for rule in rules {
                match rule {
                    Rule::Email => checks.push(quote! {
                        if !self.#ident.is_empty() && !self.#ident.contains('@') {
                            errors.push(#field_name, "invalid email");
                        }
                    }),
                    Rule::NotEmpty => checks.push(quote! {
                        if self.#ident.is_empty() {
                            errors.push(#field_name, "must not be empty");
                        }
                    }),
                    Rule::Length { min, max } => {
                        if let Some(n) = min {
                            checks.push(quote! {
                                if self.#ident.chars().count() < #n {
                                    errors.push(#field_name, format!("must be at least {} characters", #n));
                                }
                            });
                        }
                        if let Some(n) = max {
                            checks.push(quote! {
                                if self.#ident.chars().count() > #n {
                                    errors.push(#field_name, format!("must be at most {} characters", #n));
                                }
                            });
                        }
                    }
                    Rule::Range { min, max } => {
                        if let Some(n) = min {
                            checks.push(quote! {
                                if (self.#ident as f64) < #n {
                                    errors.push(#field_name, format!("must be at least {}", #n));
                                }
                            });
                        }
                        if let Some(n) = max {
                            checks.push(quote! {
                                if (self.#ident as f64) > #n {
                                    errors.push(#field_name, format!("must be at most {}", #n));
                                }
                            });
                        }
                    }
                }
            }
            Some(quote! { #(#checks)* })
        })
        .collect()
}

fn parse_rules(field: &Field) -> syn::Result<Vec<Rule>> {
    let mut out = Vec::new();
    for attr in &field.attrs {
        if !is_validate(attr) {
            continue;
        }
        let Meta::List(list) = &attr.meta else {
            continue;
        };
        let metas = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in metas {
            match meta {
                Meta::Path(path) => {
                    let name = path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                    match name.as_str() {
                        "email" => out.push(Rule::Email),
                        "not_empty" => out.push(Rule::NotEmpty),
                        other => {
                            return Err(syn::Error::new_spanned(
                                path,
                                format!("unknown validation rule `{other}`"),
                            ));
                        }
                    }
                }
                Meta::List(list) => {
                    let Some(name) = list.path.get_ident() else {
                        continue;
                    };
                    match name.to_string().as_str() {
                        "length" => out.push(parse_length(
                            list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated),
                        )),
                        "range" => out.push(parse_range(
                            list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated),
                        )),
                        other => {
                            return Err(syn::Error::new_spanned(
                                list,
                                format!("unknown validation rule `{other}`"),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

fn is_validate(attr: &Attribute) -> bool {
    attr.path().is_ident("validate")
}

fn parse_length(result: syn::Result<Punctuated<Meta, Token![,]>>) -> Rule {
    let mut min = None;
    let mut max = None;
    if let Ok(metas) = result {
        for m in metas {
            if let Meta::NameValue(nv) = m {
                if let Some(v) = int_value(&nv.value) {
                    if nv.path.is_ident("min") {
                        min = Some(v);
                    } else if nv.path.is_ident("max") {
                        max = Some(v);
                    }
                }
            }
        }
    }
    Rule::Length { min, max }
}

fn parse_range(result: syn::Result<Punctuated<Meta, Token![,]>>) -> Rule {
    let mut min = None;
    let mut max = None;
    if let Ok(metas) = result {
        for m in metas {
            if let Meta::NameValue(nv) = m {
                if let Some(v) = float_value(&nv.value) {
                    if nv.path.is_ident("min") {
                        min = Some(v);
                    } else if nv.path.is_ident("max") {
                        max = Some(v);
                    }
                }
            }
        }
    }
    Rule::Range { min, max }
}

fn int_value(expr: &Expr) -> Option<usize> {
    if let Expr::Lit(lit) = expr {
        if let Lit::Int(i) = &lit.lit {
            return i.base10_parse().ok();
        }
    }
    None
}

fn float_value(expr: &Expr) -> Option<f64> {
    if let Expr::Lit(lit) = expr {
        match &lit.lit {
            Lit::Int(i) => i.base10_parse().ok(),
            Lit::Float(f) => f.base10_parse().ok(),
            _ => None,
        }
    } else {
        None
    }
}

fn compile_error(field: &Field, msg: &str) -> TokenStream {
    syn::Error::new_spanned(field, msg).to_compile_error()
}
