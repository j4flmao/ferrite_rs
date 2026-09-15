use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item_fn: syn::ItemFn =
        syn::parse2(input).expect("`#[bootstrap]` can only be applied to `async fn main`");

    item_fn.sig.asyncness = None;
    item_fn.attrs.retain(|a| a.path().is_ident("allow"));

    let inner = item_fn.block.clone();

    item_fn.block = syn::parse_quote! {
        {
            ::ferrite_framework::__export::load_env_file();
            ::ferrite_framework::__export::tokio::runtime::Runtime::new()
                .expect("failed to start tokio runtime")
                .block_on(async move { #inner })
        }
    };

    let expanded = quote! { #item_fn };

    expanded
}
