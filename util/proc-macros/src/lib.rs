mod to_static;

use proc_macro::TokenStream;

#[proc_macro_derive(ToStatic, attributes(to_static))]
pub fn derive_to_static(tokens: TokenStream) -> TokenStream {
    match to_static::derive_to_static(tokens.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
