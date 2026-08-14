//! Derive and attribute macros for [`verum`](https://docs.rs/verum).
//!
//! Depend on `verum` rather than this crate; it re-exports everything defined
//! here. The split exists because a `proc-macro = true` crate cannot export
//! anything besides procedural macros.
//!
//! # Status
//!
//! No macros are defined yet. `#[endpoint]`, `#[contract]`, and
//! `#[derive(Domain)]` arrive in Phase 2.
