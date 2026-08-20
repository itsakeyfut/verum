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
    let m = syn::Ident::new(
        &format!("__verum_{}", name.to_string().to_lowercase()),
        name.span(),
    );
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
    // P46 — layer 1. #44 asks whether the attribute form can enforce
    // `read-contract.md`'s "forbid `Deserialize` on a domain". It can, for the
    // derives it can SEE, and P42 is the position where it cannot see them.
    if let Err(e) = reject_forbidden_derives(&ast.attrs) {
        return e.to_compile_error().into();
    }
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
    // Per field, not a hardcoded `email()`. The hardcoded form made every probe
    // whose Domain had a different field name fail with unrelated `E0609`/`E0560`
    // noise, which is why #44's review had to build a parallel macro to measure
    // anything but `email` (and why the `Copy` rows below could not be written).
    let getters = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(pub fn #i(&self) -> &#t { &self.0.#i })
    });
    // `load` was hardcoded to `email: &str` for the same reason the getter was.
    // Taking the fields by value keeps the legitimate route usable for any Domain
    // shape, which is what P48/P49 need in order to exist at all.
    let params = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(#i: #t)
    });
    let inits = named.named.iter().map(|f| {
        let i = &f.ident;
        quote!(#i)
    });
    let expanded = quote! {
        #[allow(non_camel_case_types, dead_code, clippy::all)]
        mod #m {
            // NO visibility modifier, and NOT re-exported below. ADR-0010's
            // listing annotates this exact line "module-private: paths 3/4 shut
            // with it", and the ledger states those paths' closing condition as
            // "once the `Repr` carries no visibility modifier".
            //
            // `pub(super)` was here and review caught it. It is visible to the
            // PARENT module — the user's — so at the crate root it is `pub(crate)`
            // and the `Repr` is nameable crate-wide (measured: compiles; with no
            // modifier, `E0603`). That puts the exposure radius back under the
            // user's layout, which is the one property ADR-0010 chose option E to
            // eliminate. The token came from ADR-0011's P40 sketch, where the
            // module holds only an `impl` and it is harmless.
            #fwd
            struct #repr { #(#fields),* }

            pub struct #name(#repr);

            impl #name {
                // No modifier: visible only inside this module. ADR-0010's wall.
                fn from_repr(r: #repr) -> Self { Self(r) }
                #(#getters)*
            }

            // ── THE POSITION-INDEPENDENT HALF (#44 review, C2) ────────────────
            // The name-based check below cannot see a derive written ABOVE the
            // attribute, and cannot see `r#Default` or `use .. as Dup` in ANY
            // position. Occupying the coherence slot does both: the user's derive
            // collides with `E0119`, spanned on their own derive, in every
            // position and under every spelling — coherence does not read names.
            //
            // The bodies are `unimplemented!()`, so this is not a checked
            // alternative: a legitimate `Default::default()` compiles and panics.
            // That is the cost, and it is why the check below is kept for the one
            // position where it can produce a real message.
            impl ::core::default::Default for #name {
                fn default() -> Self {
                    unimplemented!(
                        "a Domain is built by the repository #[domain] generates \
                         beside it, never by Default::default() (ledger path 26)"
                    )
                }
            }
            impl ::core::clone::Clone for #name {
                fn clone(&self) -> Self {
                    unimplemented!(
                        "a Domain is not duplicated; hand out a Projection instead \
                         (ledger paths 3 and 26)"
                    )
                }
            }

            pub struct #repository;

            impl #repository {
                /// Builds the `Repr` **inside** the module, so no caller outside can
                /// supply invented values. The public `build(r: Repr)` this replaced
                /// was the forge factory review found.
                pub fn load(&self, #(#params),*) -> #name {
                    #name::from_repr(#repr { #(#inits),* })
                }
            }
        }
        #vis use #m::{#name, #repository};
    };
    expanded.into()
}

/// Derives that hand out a Domain with no capability. Each is a route ledger
/// path 26 records, and each is measured: `Default` invents one, `Clone` takes an
/// owned copy, `Deserialize` sets every field from a string.
///
/// `Debug` is absent because path 4's remedy is a *derive-generated* `Debug` that
/// prints only the declared fields, and it does not exist yet — this spike emits
/// none, so the `E0119` an earlier version of this comment gave as the reason is not
/// available here. Unverified as a rationale; when that `Debug` lands, `Debug` joins
/// the collision list rather than this one.
const FORBIDDEN_DERIVES: &[&str] = &["Default", "Clone", "Deserialize"];

/// Rejects a forbidden derive **that this attribute can see, by its spelling**.
///
/// THE LIMITS ARE THE POINT, AND BOTH ARE MEASURED
///   *Position*: this only sees derives written **below** `#[domain]` (P46).
///   rustc expands an item's outer attributes in source order and the first
///   active attribute macro consumes the rest, so a derive written **above** is
///   expanded first and never reaches this function's token stream at all — it
///   then applies to whatever the attribute emitted (P42's `E0560`).
///
///   *Spelling*: it matches the path's last segment, so `r#Default` slipped
///   through until `unraw` was added, and an aliased import still does — a proc
///   macro sees tokens and resolves nothing (#44 review, compile- and
///   run-verified).
///
///   **Neither limit applies to the conflicting impls the expansion emits**
///   (P47/P48). Those are what actually close `Default` and `Clone`; this check
///   survives because at the below position it turns a silent no-op into a
///   message naming the route, and because `Deserialize` cannot be closed by
///   collision — verum cannot name a trait from a crate it does not depend on.
///   ADR-0015 records the split.
fn reject_forbidden_derives(attrs: &[syn::Attribute]) -> syn::Result<()> {
    let mut err: Option<syn::Error> = None;
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let list = match attr.meta.require_list() {
            Ok(l) => l,
            Err(_) => continue,
        };
        let paths = match list.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) {
            Ok(p) => p,
            Err(_) => continue,
        };
        for path in paths {
            // The last segment, so `serde::Deserialize` and a bare `Deserialize`
            // are the same finding. Matching the whole path would let one import
            // form through.
            let Some(last) = path.segments.last() else { continue };
            // `unraw`: `to_string()` on a raw ident yields `"r#Default"`, which
            // matched nothing — one token defeated the whole list (#44 review).
            // Unrawing closes that spelling and NOT the general problem: an
            // aliased import (`use core::clone::Clone as Dup;`) is invisible to
            // any name match, because a proc macro sees tokens and never resolves
            // them. That is the same argument this project uses to reject path 5's
            // name-based field whitelist, and it applies here too (SRK-004).
            let name = syn::ext::IdentExt::unraw(&last.ident).to_string();
            if !FORBIDDEN_DERIVES.contains(&name.as_str()) {
                continue;
            }
            // Spanned on the derive itself, not on the attribute: `proc-macro.md`
            // forbids `call_site()`, and the fix is at the derive.
            let e = syn::Error::new_spanned(
                &path,
                format!(
                    "`{name}` cannot be derived on a Domain: it hands out a value with no \
                     capability, so `{name}::…` is callable from any crate (ledger path 26). \
                     Remove the derive, or build the Domain through the repository \
                     `#[domain]` generates beside it."
                ),
            );
            match &mut err {
                Some(acc) => acc.combine(e),
                None => err = Some(e),
            }
        }
    }
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// P43 — a **shape-preserving** `#[domain]` in the user's own module: private
/// named fields, struct left where the user wrote it.
///
/// This is the shape `mutation-contract.md` describes, and it is the variable P44
/// isolates. It deliberately does NOT call `reject_forbidden_derives`, because the
/// derive under test is written above it and would be invisible either way — the
/// question is what happens when nothing rejects it.
#[proc_macro_attribute]
pub fn domain_keep_shape(_attr: TokenStream, item: TokenStream) -> TokenStream {
    keep_shape(item, false)
}

/// P44 — the same shape, emitted into a **macro-owned child module** (ADR-0010's
/// confinement radius), and re-exported.
///
/// The only difference from `domain_keep_shape` is placement. That is what makes
/// the pair a measurement rather than an anecdote.
#[proc_macro_attribute]
pub fn domain_keep_shape_confined(_attr: TokenStream, item: TokenStream) -> TokenStream {
    keep_shape(item, true)
}

fn keep_shape(item: TokenStream, confine: bool) -> TokenStream {
    let ast: syn::ItemStruct = match syn::parse(item) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let name = &ast.ident;
    let vis = &ast.vis;
    let named = match &ast.fields {
        syn::Fields::Named(f) => f,
        other => {
            return syn::Error::new_spanned(other, "needs a struct with named fields")
                .to_compile_error()
                .into();
        }
    };
    // `crate::`-qualified in the probe source, because moving a struct into a
    // generated module does not rewrite its field type paths — a real defect of
    // this shape, recorded in #34's review as the `E0425` it produced.
    let fields = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(#i: #t)
    });
    let gets = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(pub fn #i(&self) -> &#t { &self.#i })
    });
    let inits = named.named.iter().map(|f| {
        let i = &f.ident;
        quote!(#i: Default::default())
    });
    let body = quote! {
        pub struct #name { #(#fields),* }
        impl #name {
            #(#gets)*
            /// The legitimate route: built inside the scope that owns the fields,
            /// the way the generated repository builds one.
            pub fn load() -> Self { Self { #(#inits),* } }
        }
    };
    if !confine {
        // Same module as the user's item, so a derive written above it generates
        // its impl where the private fields ARE reachable.
        return quote! { #body }.into();
    }
    let m = syn::Ident::new(
        &format!("__verum_keep_{}", to_snake(&name.to_string())),
        name.span(),
    );
    quote! {
        #[allow(dead_code)]
        mod #m { #body }
        #vis use #m::#name;
    }
    .into()
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
        syn::Meta::List(l) if l.path.is_ident("repr_derive") => {
            reject_copy_passthrough(&l.tokens)?;
            Ok(Some(l.tokens))
        }
        other => Err(syn::Error::new_spanned(
            other,
            "#[domain] accepts only `repr_derive(..)`, e.g. \
             `#[domain(repr_derive(sqlx::FromRow))]`.",
        )),
    }
}

/// `Copy` on the `Repr` is the one pass-through that reopens path 26, and this is
/// the position where rejecting it works unconditionally.
///
/// WHY THIS ONE IS NOT POSITION-DEPENDENT
///   `repr_derive(..)` is the **attribute's own argument list**, not a sibling
///   derive, so it is always in this macro's token stream. Contrast
///   `reject_forbidden_derives`, which sees one position out of two.
///
/// WHY IT IS NEEDED AT ALL — a consequence the #44 review measured
///   `#[derive(Copy)]` on the Domain requires `Self: Clone`. Before this macro
///   emitted its own `Clone`, that requirement was unmet and `Copy` failed with
///   `E0277` — an *incidental* barrier. Emitting `Clone` to close path 26 removed
///   it, so closing two derives opened a third. What still stops `Copy` in the
///   default shape is structural: the `Repr` carries no derive, so `Copy` on the
///   newtype is `E0204`. It becomes reachable **only** when the user asks for
///   `repr_derive(Copy)` — and a bit-copy duplicates the Domain without ever
///   calling the `clone` this macro emits, so the `unimplemented!()` body is no
///   defence. Probes P48 (rejected here) and P49 (`E0204`, the structural half).
fn reject_copy_passthrough(list: &proc_macro2::TokenStream) -> syn::Result<()> {
    // `Punctuated` has no `Parse` impl; the terminated parser has to be applied
    // explicitly, the same way `reject_forbidden_derives` does it above.
    use syn::parse::Parser as _;
    let paths = syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated
        .parse2(list.clone())?;
    for path in &paths {
        let Some(last) = path.segments.last() else { continue };
        if syn::ext::IdentExt::unraw(&last.ident) == "Copy" {
            return Err(syn::Error::new_spanned(
                path,
                "`Copy` cannot be forwarded to a Domain's Repr: it makes the Domain \
                 bit-copyable, so `let stolen = *domain;` duplicates it without \
                 calling any method (ledger path 26). Forward only what persistence \
                 needs, such as `repr_derive(sqlx::FromRow)`.",
            ));
        }
    }
    Ok(())
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
            struct #repr { pub email: String }
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

/// P51 / P52 — path 28's remedy, both forms, on the same input.
///
/// The ledger's remedy column used to say the closing mechanism was `Freeze`. The
/// review measured that wrong in both directions, and these two attributes are what
/// keeps the replacement honest rather than desk analysis: the **same** field type
/// goes through a name-based check and a bound-based one.
///
/// `NAME_WHITELIST` is the shape path 5 specifies — a derive comparing field-type
/// **tokens** against a list. It cannot resolve `type AuditTrail = RefCell<..>`,
/// because a proc macro resolves nothing.
const NAME_WHITELIST: &[&str] = &["String", "u64", "i64", "bool", "Vec"];

/// P51 — the **allow-list** horn. Expected to reject an ordinary value object,
/// because a closed list of permitted field-type names cannot contain the user's own
/// types. That is the "too narrow to allow user value objects" half of path 5's
/// dilemma, and it is why the remedy was never implementable as written.
#[proc_macro_attribute]
pub fn domain_name_checked(_a: TokenStream, item: TokenStream) -> TokenStream {
    let ast: syn::ItemStruct = match syn::parse(item) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let named = match &ast.fields {
        syn::Fields::Named(f) => f,
        other => {
            return syn::Error::new_spanned(other, "needs named fields")
                .to_compile_error()
                .into();
        }
    };
    for f in &named.named {
        // The whole point: this sees `AuditTrail`, never `RefCell`.
        let syn::Type::Path(tp) = &f.ty else { continue };
        let Some(last) = tp.path.segments.last() else { continue };
        let name = syn::ext::IdentExt::unraw(&last.ident).to_string();
        if !NAME_WHITELIST.contains(&name.as_str()) {
            return syn::Error::new_spanned(
                &f.ty,
                format!("`{name}` is not an allowed Domain field type (name-based check)"),
            )
            .to_compile_error()
            .into();
        }
    }
    let vis = &ast.vis;
    let name = &ast.ident;
    let fields = named.named.iter();
    let gets = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(pub fn #i(&self) -> &#t { &self.#i })
    });
    quote! { #vis struct #name { #(#fields),* } impl #name { #(#gets)* } }.into()
}

/// P52 — the bound-based form. **The macro passes the tokens straight into a bound
/// position and rustc resolves the alias**, so the same input is rejected.
///
/// `Sync` stands in for the real predicate: it rejects `Cell` / `RefCell` and it
/// does **not** reject `Mutex` / atomics, which is exactly the partial coverage the
/// ledger now records. It also rejects `Rc`, which is the priced cost.
#[proc_macro_attribute]
pub fn domain_bound_checked(_a: TokenStream, item: TokenStream) -> TokenStream {
    let ast: syn::ItemStruct = match syn::parse(item) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let named = match &ast.fields {
        syn::Fields::Named(f) => f,
        other => {
            return syn::Error::new_spanned(other, "needs named fields")
                .to_compile_error()
                .into();
        }
    };
    let asserts = named.named.iter().map(|f| {
        let t = &f.ty;
        quote! {
            const _: () = {
                fn assert_allowed<T: ::core::marker::Sync + ?Sized>() {}
                let _ = assert_allowed::<#t>;
            };
        }
    });
    let vis = &ast.vis;
    let name = &ast.ident;
    let fields = named.named.iter();
    let gets = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(pub fn #i(&self) -> &#t { &self.#i })
    });
    quote! {
        #(#asserts)*
        #vis struct #name { #(#fields),* }
        impl #name { #(#gets)* }
    }
    .into()
}

/// The **deny-list** horn: names known to carry interior mutability. This is the form
/// that looks workable, and P53 is why it is not.
const NAME_DENYLIST: &[&str] = &["RefCell", "Cell", "Mutex", "RwLock", "UnsafeCell"];

/// P53 — the deny-list horn. Expected to **accept** the alias, which is the defect:
/// the macro compares the token `Audit` against the list and never sees `RefCell`.
/// Together with P51 this is the whole dilemma; P52 is the way out of it.
#[proc_macro_attribute]
pub fn domain_deny_checked(_a: TokenStream, item: TokenStream) -> TokenStream {
    let ast: syn::ItemStruct = match syn::parse(item) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };
    let named = match &ast.fields {
        syn::Fields::Named(f) => f,
        other => {
            return syn::Error::new_spanned(other, "needs named fields")
                .to_compile_error()
                .into();
        }
    };
    for f in &named.named {
        let syn::Type::Path(tp) = &f.ty else { continue };
        let Some(last) = tp.path.segments.last() else { continue };
        let name = syn::ext::IdentExt::unraw(&last.ident).to_string();
        if NAME_DENYLIST.contains(&name.as_str()) {
            return syn::Error::new_spanned(
                &f.ty,
                format!("`{name}` carries interior mutability (deny-list check)"),
            )
            .to_compile_error()
            .into();
        }
    }
    let vis = &ast.vis;
    let name = &ast.ident;
    let fields = named.named.iter();
    let gets = named.named.iter().map(|f| {
        let i = &f.ident;
        let t = &f.ty;
        quote!(pub fn #i(&self) -> &#t { &self.#i })
    });
    quote! { #vis struct #name { #(#fields),* } impl #name { #(#gets)* } }.into()
}
