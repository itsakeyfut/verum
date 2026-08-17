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

/// P38 — ADR-0010's **full** shape attempted from a derive: the confinement
/// module, plus the `pub use` that makes the domain usable.
///
/// P16 already shows a derive cannot emit a sibling named after its input. This
/// goes further, because ADR-0010's shape puts the struct *inside a module* —
/// where the name does not collide. The re-export is what collides, and it is not
/// optional: without it the user's `Account` is the original transparent struct.
#[proc_macro_derive(DomainAdr0010Derive)]
pub fn domain_adr0010_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("derive input");
    let name = &ast.ident;
    let m = syn::Ident::new(&format!("__verum_{}", name.to_string().to_lowercase()), name.span());
    let repr = syn::Ident::new(&format!("{name}Repr"), name.span());
    quote! {
        mod #m {
            pub struct #repr { pub email: String }
            pub struct #name(#repr);
            impl #name { fn from_repr(r: #repr) -> Self { Self(r) } }
            pub struct Repository;
            impl Repository { pub fn build(r: #repr) -> #name { #name::from_repr(r) } }
        }
        pub use #m::{#name, #repr, Repository};
    }
    .into()
}

/// P39 — ADR-0010's shape from an **attribute**, which consumes the user's item
/// instead of adding to it.
///
/// FAITHFUL TO ADR-0010, AFTER REVIEW CORRECTED IT
///   The first version re-exported the `Repr` and exposed `pub fn build(r: Repr)`.
///   Together those were a **public forge factory**: review compiled
///   `Repository::build(Repr { .. })` from the app crate *and from a foreign
///   crate*, reopening ledger path 21 wider than the ledger records it. ADR-0010's
///   listing has the `Repr` module-private and not re-exported, and the repository
///   building it **inside** the module — which is what `app/src/nested.rs` (the
///   hand-written reference P31/P32/P34/P35 measure) already did. This now matches.
///
///   The Repr stays nameable to sqlx because the query happens inside the module,
///   which is the same reason the reference works.
///
/// `repr_derive(..)` is the pass-through. It is parsed, not spliced raw — the
/// documented syntax did not compile before review.
#[proc_macro_attribute]
pub fn domain_attr(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr2 = proc_macro2::TokenStream::from(attr);
    // Layer 1, not a panic. `.expect()` produced `custom attribute panicked` with
    // the span collapsed onto the whole attribute — `proc-macro.md` forbids that,
    // and it was the #37 defect recurring one PR later.
    let ast: syn::ItemStruct = match syn::parse(item) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let name = &ast.ident;
    let vis = &ast.vis;
    let named = match &ast.fields {
        syn::Fields::Named(f) => f,
        other => {
            return syn::Error::new_spanned(
                other,
                "#[domain] needs a struct with named fields: the field names are what \
                 the Field markers and the `pub`-field check are generated from.",
            )
            .to_compile_error()
            .into();
        }
    };
    // `repr_derive(A, B)` -> `#[derive(A, B)]` on the Repr. Parsed so the syntax the
    // specs document is the syntax that works.
    let fwd = match parse_repr_derive(&attr2) {
        Ok(None) => quote!(),
        Ok(Some(list)) => quote!(#[derive(#list)]),
        Err(e) => return e.to_compile_error().into(),
    };
    let m = syn::Ident::new(
        &format!("__verum_{}", to_snake(&name.to_string())),
        name.span(),
    );
    let repr = syn::Ident::new(&format!("{name}Repr"), name.span());
    // Per-type, as ADR-0010's listing writes it. A fixed `Repository` collided on
    // the second domain in a module (E0252).
    let repository = syn::Ident::new(&format!("{name}Repository"), name.span());
    // The user's field attributes are forwarded: `#[sqlx(rename = "..")]` is how a
    // column is mapped, and dropping it silently broke the sqlx route.
    let fields = named.named.iter().map(|f| {
        let attrs = &f.attrs;
        let i = &f.ident;
        let t = &f.ty;
        quote!(#(#attrs)* pub #i: #t)
    });
    let expanded = quote! {
        #[allow(non_camel_case_types, dead_code, clippy::all)]
        mod #m {
            // NOT `pub`, and NOT re-exported below. ADR-0010: "module-private:
            // paths 3/4 shut with it".
            #fwd
            pub(super) struct #repr { #(#fields),* }

            pub struct #name(#repr);

            impl #name {
                // No modifier: visible only inside this module. ADR-0010's wall.
                fn from_repr(r: #repr) -> Self { Self(r) }
                pub fn email(&self) -> &str { &self.0.email }
            }

            pub struct #repository;

            impl #repository {
                /// Builds the `Repr` **inside** the module, so no caller outside can
                /// supply invented values. The public `build(r: Repr)` this replaced
                /// was the forge factory review found.
                pub fn load(&self, email: &str) -> #name {
                    #name::from_repr(#repr { email: email.to_owned() })
                }
            }
        }
        #vis use #m::{#name, #repository};
    };
    expanded.into()
}

/// `repr_derive(A, B)` -> the inner list. Anything else is a layer-1 error rather
/// than a confusing downstream one.
fn parse_repr_derive(
    attr: &proc_macro2::TokenStream,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    if attr.is_empty() {
        return Ok(None);
    }
    let meta: syn::Meta = syn::parse2(attr.clone())?;
    match meta {
        syn::Meta::List(l) if l.path.is_ident("repr_derive") => Ok(Some(l.tokens)),
        other => Err(syn::Error::new_spanned(
            other,
            "#[domain] accepts only `repr_derive(..)`, e.g. \
             `#[domain(repr_derive(sqlx::FromRow))]`.",
        )),
    }
}

/// `FooBar` -> `foo_bar`. Plain lowercasing collapsed `FooBar` and `Foobar` onto
/// one module name (E0428).
fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// P40 — **a derive CAN own ADR-0010's confinement radius.**
///
/// The trick review found: emit only the `impl` block into the generated module.
/// A private inherent method's visibility is the module the **`impl`** is written
/// in, not where the type is defined — so no re-export is needed and nothing
/// collides with the user's item.
///
/// This refutes "a derive cannot produce it" (ADR-0011's original mechanism). What
/// a derive cannot do is **consume** the user's item, so the transparent original
/// survives with its `pub` fields assignable — probe P40b, which is the real reason
/// the attribute form was chosen.
#[proc_macro_derive(DomainImplOnlyDerive)]
pub fn domain_impl_only_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("derive input");
    let name = &ast.ident;
    let m = syn::Ident::new(
        &format!("__verum_impl_only_{}", to_snake(&name.to_string())),
        name.span(),
    );
    let repr = syn::Ident::new(&format!("{name}Repr"), name.span());
    quote! {
        #[allow(dead_code)]
        mod #m {
            pub(super) struct #repr { pub email: String }
            // The impl block lives HERE, so `from_repr`'s visibility is this module.
            impl super::#name {
                fn from_repr(r: #repr) -> Self { Self { email: r.email } }
            }
            pub struct Confined;
            impl Confined {
                pub fn load(&self, email: &str) -> super::#name {
                    super::#name::from_repr(#repr { email: email.to_owned() })
                }
            }
        }
    }
    .into()
}
