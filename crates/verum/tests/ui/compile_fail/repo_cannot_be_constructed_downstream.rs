//! A capability handle cannot be constructed outside the crate that owns it.
//!
//! `crates/verum/src/capability.rs`'s docblock rests on this: *"this type is
//! declarable and bindable but not constructible outside the crate"*. That was a
//! stated guarantee with no fixture until #39's review pointed out the gap — and
//! it is the guarantee the whole capability model depends on, because a `Repo`
//! anyone can mint is not a capability.
//!
//! Both fields are `PhantomData`, which is freely constructible, so the privacy of
//! the *fields* is what closes this rather than anything about their types.
//!
//! Refuted attacks, for the record — none of these needs a fixture because none
//! compiles: `Default::default()` is `E0277` (no `Default` impl), a functional
//! update `Repo { ..r }` is `E0451` for the same reason as the literal, and there
//! is no `Clone`. `unsafe` routes (`mem::zeroed`, `transmute`) do work because
//! `Repo` is a ZST; they are out of reach of any fixture and are why the field
//! #40 picks matters — a real `&'req Runtime` makes `zeroed()` a null reference.

struct User;

fn mint<'req>() -> verum::Repo<'req, User, (), ()> {
    verum::Repo {
        _req: core::marker::PhantomData,
        _model: core::marker::PhantomData,
    }
}

fn main() {}
