//! Plays the user's application crate: it defines the Domain, the Repository
//! implementation, and ordinary handler code, all in one crate. That is the
//! common shape, and it is the shape under which `pub(crate)` is widest.
pub mod confined;
pub mod derives;
pub mod domain;
pub mod handler;
pub mod nested;
pub mod repo;

// ---------------------------------------------------------------------------
// #33 (review round 2) — the CRATE ROOT layout.
//
// Review found that `confined.rs`'s mechanism has an unstated precondition: it
// only confines anything if the domain is declared in a module. Put the domain
// in `lib.rs` and "no visibility modifier" is *identical to* `pub(crate)`,
// because the module IS the crate. For a single-crate PoC application that is a
// perfectly ordinary layout, and nothing in the ADR, the specs or the ledger said
// so. P32 and P33 are the pair that pins it.
// ---------------------------------------------------------------------------

/// P33 — the flat mechanism at the crate root. **Must compile**: that is the hole.
#[cfg(feature = "p33-root-flat")]
pub struct RootUser(RootUserRepr);

#[cfg(feature = "p33-root-flat")]
pub(crate) struct RootUserRepr {
    pub email: String,
}

#[cfg(feature = "p33-root-flat")]
impl RootUser {
    /// "No modifier" — but this module is the crate root, so it means `pub(crate)`.
    fn from_repr(r: RootUserRepr) -> Self {
        Self(r)
    }
    pub fn email(&self) -> &str {
        &self.0.email
    }
}

/// P32 — the nested mechanism at the same crate root. **Must be rejected.**
///
/// The derive owns the module, so where the user put the domain stops mattering.
#[cfg(feature = "p32-root-nested")]
mod __verum_rootuser {
    pub struct RootNested(RootNestedRepr);
    struct RootNestedRepr {
        pub email: String,
    }
    impl RootNested {
        fn from_repr(r: RootNestedRepr) -> Self {
            Self(r)
        }
        pub fn email(&self) -> &str {
            &self.0.email
        }
    }
    pub struct RootNestedRepository;
    impl RootNestedRepository {
        pub fn load(&self) -> RootNested {
            RootNested::from_repr(RootNestedRepr {
                email: "db@example.com".to_owned(),
            })
        }
    }
}

#[cfg(feature = "p32-root-nested")]
pub use __verum_rootuser::{RootNested, RootNestedRepository};

// ---------------------------------------------------------------------------
// WHICH MACRO FORM CAN PRODUCE ADR-0010's SHAPE? (#34)
//
// ADR-0010 chose "the constructor lives in a macro-owned module". Its text calls
// that module "derive-owned" throughout, which had never been compiled.
//
// REVIEW CORRECTED THE ANSWER'S REASON, NOT THE ANSWER.
//   P38 shows a derive cannot emit the shape *as ADR-0010 writes it*, because the
//   re-export collides with the user's own item. But P40 shows a derive CAN own the
//   confinement radius — emit only the `impl` block into the module and no
//   re-export is needed. What a derive cannot do is **consume the user's item**, so
//   the transparent original survives beside the opaque one: P40b. That is the real
//   reason the attribute form wins, and it is a *cost*, not an impossibility.
// ---------------------------------------------------------------------------

/// P38 — ADR-0010's shape, verbatim, from a derive. Expected to **fail**, `E0255`.
#[cfg(feature = "p38-adr0010-from-derive")]
#[derive(mac::DomainAdr0010Derive)]
pub struct Adr0010Derive {
    pub email: String,
}

/// P39 — the same from an attribute. Expected to **compile**.
///
/// The body below is what stops this being a hollow row: review mutated the macro
/// to emit **nothing** and P39 stayed green, because a bare struct definition
/// references none of the generated items. Naming `Adr0010AttrRepository` and the
/// getter makes an empty expansion `E0433`/`E0599`.
#[cfg(feature = "p39-adr0010-from-attribute")]
#[mac::domain_attr]
pub struct Adr0010Attr {
    pub email: String,
}

#[cfg(feature = "p39-adr0010-from-attribute")]
pub fn p39_uses_the_expansion() -> String {
    Adr0010AttrRepository
        .load("db@example.com".to_owned())
        .email()
        .to_owned()
}

/// P39b — the forgery ADR-0010 exists to reject. Expected to **fail**, `E0624`.
#[cfg(feature = "p39b-attribute-forgery")]
pub fn p39b_forge() -> Adr0010Attr {
    Adr0010Attr::from_repr(todo!())
}

/// P39d — **the `Repr` is not nameable from outside**, by its *visibility*.
/// Expected to **fail**, `E0603`.
///
/// This row is **path-qualified on purpose.** The first version wrote the
/// unqualified name and got `E0422`, which only measures "the Repr was left out of
/// the re-export list" — it stayed green while the macro emitted
/// `pub(super) struct Repr`, i.e. a Repr reachable from anywhere in the user's
/// crate. Review mutation-verified that blindness. Naming the module path makes the
/// visibility itself the thing under test.
#[cfg(feature = "p39d-repr-not-nameable")]
pub fn p39d_name_the_repr() {
    let _ = crate::__verum_adr0010_attr::Adr0010AttrRepr {
        email: String::new(),
    };
}

/// P39e — the same from **another module of the user's crate**, which is where
/// `pub(super)` was reachable. Expected to **fail**, `E0603`.
///
/// P33's lesson transposed onto the `Repr`: at the crate root `pub(super)` *is*
/// `pub(crate)`, so a row that only looks from the declaring scope cannot see it.
#[cfg(feature = "p39e-repr-not-nameable-elsewhere")]
pub mod p39e_elsewhere {
    pub fn touch() {
        let _ = crate::__verum_adr0010_attr::Adr0010AttrRepr {
            email: String::new(),
        };
    }
}

/// P39c — the legitimate route, so P39b/P39d are not passing because the module is
/// unreachable. Expected to **compile**, and it has its own feature so it can be a
/// `pass` row (review: it previously shared P39b's failing build and could not be
/// one).
#[cfg(feature = "p39c-legitimate-route")]
pub fn p39c_legitimate() -> String {
    Adr0010AttrRepository
        .load("alice@example.com".to_owned())
        .email()
        .to_owned()
}

/// P40 — **a derive CAN own the confinement radius.** Expected to **compile**.
///
/// Emit only the `impl` block into the generated module: a private inherent
/// method's visibility is the module the `impl` is written in, not where the type is
/// defined. No re-export, so nothing collides. This refutes "a derive cannot
/// produce it".
#[cfg(feature = "p40-derive-can-confine")]
#[derive(mac::DomainImplOnlyDerive)]
pub struct ImplOnly {
    pub email: String,
}

/// P40a — and the wall still stands under it. Expected to **fail**, `E0624`.
#[cfg(feature = "p40a-derive-confine-forgery")]
pub fn p40a_forge() -> ImplOnly {
    ImplOnly::from_repr(todo!())
}

/// P40b — **the cost, and the real reason the attribute form wins.** Expected to
/// **compile**, which is the finding: a derive cannot consume the user's item, so
/// the transparent original survives and its `pub` fields are assignable.
#[cfg(feature = "p40-derive-can-confine")]
pub fn p40b_transparent_original(u: &mut ImplOnly) {
    u.email = String::from("assigned directly — no capability");
}
