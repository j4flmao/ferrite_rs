use proc_macro::TokenStream;

mod bootstrap;
mod controller;
mod entity;
mod impl_controller;
mod inject;
mod injectable;
mod module;
mod validate;

#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    validate::expand(input.into()).into()
}

#[proc_macro_attribute]
pub fn module(args: TokenStream, input: TokenStream) -> TokenStream {
    module::expand(args.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn injectable(args: TokenStream, input: TokenStream) -> TokenStream {
    injectable::expand(args.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn entity(args: TokenStream, input: TokenStream) -> TokenStream {
    entity::expand(args.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn controller(args: TokenStream, input: TokenStream) -> TokenStream {
    controller::expand(args.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn impl_controller(args: TokenStream, input: TokenStream) -> TokenStream {
    impl_controller::expand(args.into(), input.into()).into()
}

#[proc_macro_attribute]
pub fn inject(_args: TokenStream, input: TokenStream) -> TokenStream {
    inject::expand(input.into()).into()
}

#[proc_macro_attribute]
pub fn bootstrap(args: TokenStream, input: TokenStream) -> TokenStream {
    bootstrap::expand(args.into(), input.into()).into()
}

// Route method markers — interpreted by `#[impl_controller]`.
// When used standalone they expand the method unchanged so that a missing
// `impl_controller` is still a compile error elsewhere, not a silent no-op.

#[proc_macro_attribute]
pub fn get(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn post(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn put(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn patch(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn delete(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn all(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

// Non-functional markers for v0.2 (guards / interceptors / middleware /
// filters). These are stripped by the *impl_controller* macro; the individual
// macros are no-ops.

#[proc_macro_attribute]
pub fn use_guards(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn use_interceptors(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn use_middleware(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn catch(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}
