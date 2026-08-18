//! A capability handle cannot outlive the request that granted it.
//!
//! This is the Confirmation [ADR-0005] said it was missing: *"When #39 decides, a
//! `compile_fail` fixture for 'handle moved past the request boundary' is the
//! confirmation this ADR is missing."*
//!
//! Before #39, `Repo` had no lifetime parameter, so it was `'static` and this file
//! **compiled** — the escape was demonstrated at run time in
//! `spikes/ctx-lifetime-rpitit` (probe E1: the response returned, and the store
//! read `escaped@example.com` 150 ms later).
//!
//! Deliberately a bare `'static` bound rather than `thread::spawn`. `spawn`
//! requires `Send` **and** `'static`, so a fixture built on it would also be
//! asserting the handle's `Send`-ness, and an error on either half would look the
//! same. Here only one property is under test.
//!
//! [ADR-0005]: ../../../../../docs/adr/0005-repo-handle-shape.md

fn needs_static<T: 'static>(_: T) {}

struct User;

fn escapes<'req>(r: verum::Repo<'req, User, (), ()>) {
    needs_static(r);
}

fn main() {}
