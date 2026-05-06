//! Derive macros for Leaven artifacts and surfaces.

extern crate proc_macro;

use proc_macro::TokenStream;

mod unimplemented;

/// Derives `Artifact`.
///
/// This macro is reserved for the spec-defined derive contract, but the
/// implementation is not available yet.
#[proc_macro_derive(Artifact, attributes(leaven, content_skip))]
pub fn derive_artifact(input: TokenStream) -> TokenStream {
    unimplemented::derive("Artifact", input)
}

/// Derives `ContentAddressed`.
///
/// This macro is reserved for the spec-defined derive contract, but the
/// implementation is not available yet.
#[proc_macro_derive(ContentAddressed, attributes(leaven, content_skip))]
pub fn derive_content_addressed(input: TokenStream) -> TokenStream {
    unimplemented::derive("ContentAddressed", input)
}

/// Derives `EditSurface`.
///
/// This macro is reserved for the spec-defined derive contract, but the
/// implementation is not available yet.
#[proc_macro_derive(EditSurface, attributes(leaven_surface))]
pub fn derive_edit_surface(input: TokenStream) -> TokenStream {
    unimplemented::derive("EditSurface", input)
}
