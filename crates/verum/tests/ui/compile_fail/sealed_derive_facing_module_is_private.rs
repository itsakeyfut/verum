//! The *second* seal module must be unreachable too — and it is the one M2 exposes.
//!
//! T-M0-09 split the seals in two: `private` holds the **structural** ones
//! (`SealedConsList` / `SealedIndex` / `SealedHas` / `SealedAppend` / `SealedLookup`),
//! which verum implements itself over tuples and no derive ever will;
//! `derive_facing` holds the ones a derive must satisfy per declaration
//! (`SealedIncludes` today).
//!
//! The split exists because `api-surface.md` §2 already records that M2 *must*
//! re-export a `#[doc(hidden)] pub mod __private` for generated code to name — and
//! with all six seals in one module, that single change would make every one of
//! them nameable, reopening ledger paths 14a–14e at once. Simulated during #9's
//! review: a false membership compiled downstream.
//!
//! So this fixture pins the thing the split is *for*: `derive_facing` is private
//! today. When M2 opens it, this file's `.stderr` is what will change, which is the
//! signal that path 13's ⚠️ 暫定閉鎖 needs re-verifying — and the evidence that
//! 14a–14e were **not** dragged along with it.

use verum::derive_facing::SealedIncludes;

fn main() {}
