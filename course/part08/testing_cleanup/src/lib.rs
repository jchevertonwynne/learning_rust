use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::ItemFn;

/// Generates code that takes an `async fn(mongodb::Client, testing::config::AppConfig)`,
/// calls `common::setup()` to initialise the test with its own mongo database, performs the test & then
/// executes the cleanup future once the test has ran. This is to avoid on the repeated writing of boilerplate
/// in tests.
#[proc_macro_attribute]
pub fn test_with_cleanup(_args: TokenStream, items: TokenStream) -> TokenStream {
    let mut my_fn: ItemFn = syn::parse(items).unwrap();

    let new_ident = Ident::new("test_inner_fn", my_fn.sig.ident.span());
    let ident = std::mem::replace(&mut my_fn.sig.ident, new_ident);

    quote!(
        #[test]
        fn #ident() -> anyhow::Result<()> {
            let rt = common::rt();

            rt.block_on(async {
                let (mongo, config, cleanup) = common::setup().await?;

                #my_fn

                let res = ::tokio::spawn(test_inner_fn(mongo, config)).await;

                cleanup.await?;

                res??;

                ::core::result::Result::<(), ::anyhow::Error>::Ok(())
            })?;

            ::core::result::Result::Ok(())
        }
    )
    .into()
}
