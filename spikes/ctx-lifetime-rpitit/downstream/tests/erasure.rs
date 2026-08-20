//! The existence pin for the G series, and the reason it is a **test** and not a
//! `const _` beside the code.
//!
//! WHY THE OBVIOUS PIN DOES NOT WORK HERE
//!   `docs/rules/test.md` §9-13 prescribes `const _: fn(A) -> B = f;` at the call
//!   site, and `app/src/lib.rs` uses that form in a dozen places. It pins a
//!   *signature*. It cannot pin *existence*, because it lives inside the same
//!   `#[cfg]` as the code it points at: delete the region and the pin goes with
//!   it, leaving an empty crate that still prints `Finished`. Measured in #44's
//!   review — 90 lines of G1/G2/G3 deleted, suite still `29 as specified, 0
//!   unexpected` — and re-measured after adding `const _` pins, which changed
//!   nothing. The same is true of the pin this spike already had: re-pointing
//!   D5c's `#[cfg]` leaves **D5c** green (it reddens D5b as collateral), so
//!   `README.md`'s "the existence pin" row describes an effect it does not have.
//!
//!   What cannot pass vacuously is a needle that only real code can print. The
//!   items below are referenced from a test binary, so deleting or narrowing any
//!   of them is a compile error in *this* file, and the marker is unreachable.
use downstream::{AnyService, Email, ErasedService, User, UserService};
use verum::Repo;

#[test]
fn every_erasure_shape_resolves_against_the_shipped_repo() {
    // G1 — the user's own object-safe trait on a fully declared handle.
    const _: fn(&Repo<'_, User, (Email, ()), (Email, ())>) = |h| UserService::touch(h);
    // G2 — the blanket impl, instantiated at a concrete shape so narrowing it
    // to one shape fails here as well as deleting it.
    const _: fn(&Repo<'_, User, (Email, ()), (Email, ())>) = |h| AnyService::touch_any(h);
    // G3 — the erasure itself: a `&dyn` with no domain, field set or endpoint.
    const _: for<'a> fn(&'a Repo<'a, User, (Email, ()), (Email, ())>) -> &'a dyn ErasedService =
        downstream::erase;
    const _: fn(&dyn ErasedService) = downstream::service;

    // Three shapes, asserted as a count. `test result: ok` alone matches a deleted
    // file, and `Finished` alone matches an empty crate.
    println!("VERUM_G_ERASURE=3");
}
