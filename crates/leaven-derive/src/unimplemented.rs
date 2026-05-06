use proc_macro::TokenStream;

pub fn derive(name: &str, _input: TokenStream) -> TokenStream {
    format!(
        "compile_error!(\"#[derive({name})] is reserved by leaven-derive but is not implemented yet; write the trait impl by hand until the derive contract lands.\");"
    )
    .parse()
    .expect("reserved derive compile_error expands")
}
