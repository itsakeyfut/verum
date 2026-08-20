//! Ledger path 27 — the user erases the capability parameters themselves.
//!
//! Path 11's remedy reads "do not expose `dyn Repository`; parameterise the
//! service by capabilities too", and its state read "Closed in the First PoC".
//! Verum exposes no `dyn` anything. The remedy is still unenforceable, because
//! **the user supplies the trait**: `Repo` is public, a local trait passes the
//! orphan rule with a foreign `Self` type, and `&dyn` erases `D`, `R` and `M`.
//!
//! Nothing here is a defect in `verum::Repo`. It is what a public handle type
//! costs, and G4 is the boundary: the erasure loses the *capability parameters*
//! and does **not** defeat `'req` (path 24). Path 27 is capability erasure, path 24
//! was scope escape — the ledger keeps them apart for that reason.

use verum::Repo;

// G1, G2 and G3 are all `pass` cases, so they are **not** feature-gated: gating
// them made them undetectable if deleted. A `const _` beside gated code pins the
// signature and not the existence — delete the gate and the pin goes with it, and
// the row's `Finished` needle is satisfied by an empty crate (#44's review deleted
// all three, 90 lines, and the suite stayed at `0 unexpected`). `tests/erasure.rs`
// references them from a test binary with a count-bearing marker instead, which is
// the pin that cannot pass vacuously. Only the rejection rows (G4/G5) are gated,
// because they must not break the baseline.

/// Stand-ins for the user's own domain and field markers. Local types, which is
/// what makes every impl below legal.
pub struct User;
pub struct Email;

/// Always compiled, with every probe off.
///
/// Two jobs. It keeps the `use` above live, so the default build has no warning to
/// hide a real one behind. And it pins the handle's **arity, both kinds**: dropping
/// or adding a type parameter in `crates/verum/src/capability.rs` is `E0107` here,
/// and so is removing `'req` or adding a second lifetime — both measured. #39's
/// finding, that a lifetime argument elides silently, is about a use site that
/// **omits** it; this one writes `'r` explicitly, so it is pinned. An earlier
/// version of this comment called it a partial pin, which under-claimed it.
pub type Handle<'r, D, R, M> = Repo<'r, D, R, M>;

type FullyDeclared<'r> = Repo<'r, User, (Email, ()), (Email, ())>;

/// G1 — one concrete, fully parameterised handle behind the user's own trait.
///
/// The trait is object-safe because the user wrote it that way; verum has no say.
pub trait UserService {
    fn touch(&self);
}

impl UserService for FullyDeclared<'_> {
    fn touch(&self) {}
}

/// §9-13. A trait plus an impl and no caller compiles just as well with the impl
/// deleted, so the row would stay green with nothing measured — verified by
/// deleting it. The call is what makes G1 depend on the impl resolving.
pub fn call_it(handle: &FullyDeclared<'_>) {
    handle.touch();
}


/// G2 — the same thing as a **blanket** impl. One line covers every capability
/// shape there is, present and future, including sets the endpoint never declared.
///
/// This is why path 27's remedy cannot be enforced by narrowing what verum emits:
/// there is no set of exports that makes this impl illegal without making `Repo`
/// itself unnameable.
pub trait AnyService {
    fn touch_any(&self);
}

impl<D, R, M> AnyService for Repo<'_, D, R, M> {
    fn touch_any(&self) {}
}

/// The caller, generic over every capability shape — which is the claim. If the
/// blanket impl is narrowed to one shape this stops resolving, so the row measures
/// the coverage and not merely that an impl was written.
pub fn call_any<D, R, M>(handle: &Repo<'_, D, R, M>) {
    handle.touch_any();
}


/// G3 — the service signature the contract can no longer see.
///
/// `service` takes no domain, no field set, and no endpoint. Whatever the AI
/// Context says the endpoint reads and mutates, this function's type says nothing,
/// and the transitive closure `effect-inference.md` would need cannot be taken
/// from a `&dyn`.
pub trait ErasedService {
    fn run(&self);
}

impl<D, R, M> ErasedService for Repo<'_, D, R, M> {
    fn run(&self) {}
}

/// The service the contract can no longer describe: no domain, no field set, no
/// endpoint in the signature.
pub fn service(handle: &dyn ErasedService) {
    handle.run();
}

/// §9-13, the second time. An earlier version put the coercion in the body, and
/// replacing it with a direct `handle.run()` left the row green — G3 was measuring
/// "the impl resolves", which G1 and G2 already measure. Naming `&dyn` in the
/// return type fixes that.
///
/// What is NOT claimed: that this is the only place the coercion can happen.
/// Making this function return `&Repo` again keeps the row green, because
/// `service`'s parameter then coerces at the call instead — the finding survives
/// the move, which is the point. The row's non-vacuity rests on deleting the
/// blanket impl, which does turn it red (measured).
pub fn erase<'a, D, R, M>(handle: &'a Repo<'_, D, R, M>) -> &'a dyn ErasedService {
    handle
}

pub fn erase_and_run<D, R, M>(handle: &Repo<'_, D, R, M>) {
    service(erase(handle));
}


/// G4 — the control, and the boundary of what path 27 claims.
///
/// If the erasure also laundered `'req` into `'static`, path 27 would dominate
/// path 24 and #39's closure would be worthless. It does not: `dyn Trait` erases
/// the *type* arguments and still carries a lifetime bound, so a `&'static dyn`
/// cannot be produced from a handle borrowed for the request.
///
/// Expected to FAIL, **with a lifetime error and nothing else**. An earlier version
/// of this probe returned `*self` from a `&self` method and collected `E0507`
/// alongside the lifetime error — it would then have stayed red after the lifetime
/// rule was removed, which is the "rejected for the wrong reason" shape this
/// spike's `probe` needle exists to catch.
#[cfg(any(feature = "g4-erasure-does-not-defeat-req", feature = "g5-owned-erasure-needs-static"))]
pub trait Escaping {
    fn run(&self);
}

#[cfg(any(feature = "g4-erasure-does-not-defeat-req", feature = "g5-owned-erasure-needs-static"))]
impl<D, R, M> Escaping for Repo<'_, D, R, M> {
    fn run(&self) {}
}

#[cfg(feature = "g4-erasure-does-not-defeat-req")]
pub fn launder<D, R, M>(handle: &Repo<'_, D, R, M>) -> &'static dyn Escaping {
    handle
}

/// G5 — the same boundary, in the shape that actually depends on `Repo`'s own
/// lifetime.
///
/// **G4 alone does not measure what it claims.** #44's review deleted `'req` from
/// `Repo` entirely and G4 stayed red, because the outer `&'1` still cannot outlive
/// `'static` — the row passes on an error that has nothing to do with the handle.
/// `Box<dyn Escaping>` carries an implicit `+ 'static`, so this signature requires
/// `Repo<'r, ..>: 'static`, i.e. `'r: 'static`, and nothing supplies it. Remove
/// `'req` and this one compiles: measured, and it is what makes the pair
/// non-vacuous.
#[cfg(feature = "g5-owned-erasure-needs-static")]
pub fn stash<'r, D: 'static, R: 'static, M: 'static>(
    handle: Repo<'r, D, R, M>,
) -> Box<dyn Escaping> {
    Box::new(handle)
}
