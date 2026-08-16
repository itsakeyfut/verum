//! Plays the user's application crate: it defines the Domain, the Repository
//! implementation, and ordinary handler code, all in one crate. That is the
//! common shape, and it is the shape under which `pub(crate)` is widest.
pub mod confined;
pub mod domain;
pub mod nested;
pub mod handler;
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
