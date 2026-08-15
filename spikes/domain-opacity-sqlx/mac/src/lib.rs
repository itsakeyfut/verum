//! Stands in for `verum-macros`, only far enough to answer one question:
//! **can a `derive` produce the shape the specs describe?**
//!
//! This crate exists because the corrected specification asserted that
//! `#[derive(Domain)]` must emit `pub struct User(UserRepr)`. That assertion was
//! written without compiling anything, and it is false — P16 below is what
//! establishes it, and P17 is the control that keeps P16 from being explained by
//! "the macro in this spike is simply broken".
use proc_macro::TokenStream;
use quote::quote;

/// P16 — emit the newtype the specs describe, alongside the user's own item.
#[proc_macro_derive(DomainNewtype)]
pub fn domain_newtype(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("derive input");
    let name = &ast.ident;
    let repr = syn::Ident::new(&format!("{name}Repr"), name.span());
    quote! {
        pub(crate) struct #repr { pub email: String }
        pub struct #name(#repr);
    }
    .into()
}

/// P17 — control: emit the `Repr` **and a newtype**, under a name that does not
/// collide with the input.
///
/// The newtype is what makes this a control. Emitting only the `Repr` would show
/// merely that the macro runs — measured: with that version, deleting the derive or
/// making the macro emit nothing both left the suite at 21/0. Emitting a newtype
/// under a free name isolates the actual difference from P16, which is **the name**,
/// not the shape: a derive can produce `struct XRepr` and `struct XWrapper`, and
/// cannot produce `struct X` because `X` is already defined.
#[proc_macro_derive(DomainReprOnly)]
pub fn domain_repr_only(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("derive input");
    let name = &ast.ident;
    let repr = syn::Ident::new(&format!("{name}Repr"), name.span());
    let wrapper = syn::Ident::new(&format!("{name}Wrapper"), name.span());
    quote! {
        pub(crate) struct #repr { pub email: String }
        pub(crate) struct #wrapper(#repr);
    }
    .into()
}
