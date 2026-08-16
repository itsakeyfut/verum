//! T-M1-03 / #15 — can capability-checked getters enforce `reads` without
//! `Projection`?
//!
//! The downstream crate: it owns the `Domain`, and `fw` owns the `Repo`. That
//! split is load-bearing — see E1.
//!
//! WHAT THE SPECS DID NOT SAY
//!   `read-contract.md:162`, `mutation-contract.md:95` and ADR-0004 all name
//!   "capability-checked getters" and none of them says **what carries `R`**.
//!   Without `Projection`, `find()` returns a `Domain`, which has no read set.
//!
//! WHAT IS DELIBERATELY REAL HERE
//!   `Has`, `Here` and `There` are **`verum`'s**, not stand-ins. Requirement 4
//!   asks for the error text, and the real `Has` carries `on_unimplemented` and
//!   `do_not_recommend`; a local copy would record a different specification
//!   from the one M4 inherits.
//!
//! WHAT IS NOT REAL HERE
//!   `Read<D, F>` and `Field` are undeclared in `verum` (ADR-0007 is
//!   `proposed`), so the effect elements below are bare marker types. `Has` is
//!   generic over its element, so nothing about the measurement depends on that
//!   choice — and using a marker keeps this spike from pre-empting #34.

use fw::{ReadSet, Repo};
use std::marker::PhantomData;
use verum::Has;

/// Field markers. In the real design these are `Read<User, user::Email>`;
/// `Has` does not care, and `Field` is ADR-0007's to declare.
pub struct ReadEmail;
pub struct ReadName;
pub struct ReadSecret;

/// The domain, opaque as T-M1-01 requires: private fields, no `pub` anywhere.
pub struct Domain {
    email: String,
    name: String,
    secret: String,
}

impl Domain {
    /// Not a getter — the constructor a repository implementation would use.
    pub fn new(email: &str, name: &str, secret: &str) -> Self {
        Self {
            email: email.to_owned(),
            name: name.to_owned(),
            secret: secret.to_owned(),
        }
    }
}

/// An endpoint that declares `reads = [email]` and nothing else.
pub type DeclaredEmailOnly = (ReadEmail, ());

/// The same, declaring two fields, so `There<_>` is exercised as well as `Here`.
pub type DeclaredEmailAndName = (ReadEmail, (ReadName, ()));

// ---------------------------------------------------------------------------
// (a) Where can the getter live at all?
// ---------------------------------------------------------------------------

/// E1 — the shape the first version of this spike measured: an inherent impl on
/// the framework's `Repo`, parameterised by the local `Domain`. Expected to
/// **fail** with `E0116`.
///
/// This is RK-004. It compiled in the single-crate version only because `Repo`
/// was local there. The whole of shape A rested on it, so the correction is not
/// cosmetic: **the measured surface could not exist.**
#[cfg(feature = "e1-inherent-impl-foreign-type")]
impl<R, M> Repo<Domain, R, M> {
    pub fn inherent_email<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        R: Has<ReadEmail, I>,
    {
        &d.email
    }
}

/// The shape that *is* available downstream: an extension trait, which is what
/// a `#[derive(Domain)]` would emit into the user's crate.
///
/// **It carries no type parameter.** `R` arrives only through `Self::Set`, and
/// that is the entire reason it cannot be forged — see F1 and F2. An earlier
/// draft wrote `trait UserRead<R>`, which is forgeable in one line.
pub trait UserRead: ReadSet {
    fn email<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadEmail, I>;

    fn name<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadName, I>;

    fn secret<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadSecret, I>;
}

impl<R, M> UserRead for Repo<Domain, R, M> {
    fn email<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadEmail, I>,
    {
        &d.email
    }

    fn name<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadName, I>,
    {
        &d.name
    }

    fn secret<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadSecret, I>,
    {
        &d.secret
    }
}

/// E2 — reading a declared field at `Here`. Expected to **compile**.
pub fn e2_declared_read(r: &Repo<Domain, DeclaredEmailOnly, ()>, d: &Domain) -> String {
    r.email(d).to_owned()
}

/// E2b — the same one element deeper, so the `There<_>` impl is covered and not
/// only `Here`. Expected to **compile**.
///
/// This is E3's rule-14 control (`docs/rules/test.md` §9-14): E3 rejects `name`
/// against a set that lacks it, and this shows the *same call* succeeds once the
/// set contains it. Without the pair, E3 could be failing because the method is
/// broken rather than because the capability is absent.
pub fn e2b_declared_read_at_depth(
    r: &Repo<Domain, DeclaredEmailAndName, ()>,
    d: &Domain,
) -> String {
    r.name(d).to_owned()
}

/// E3 — reading a field the endpoint did not declare. Expected to **fail**.
///
/// This error text is requirement 4: it is what an AI sees when it reads a field
/// outside the contract, and it seeds M4.
#[cfg(feature = "e3-undeclared-read")]
pub fn e3_undeclared_read(r: &Repo<Domain, DeclaredEmailOnly, ()>, d: &Domain) -> String {
    r.name(d).to_owned()
}

// ---------------------------------------------------------------------------
// (b) Is the extension trait forgeable?
// ---------------------------------------------------------------------------

/// F1 — the parameterised form, `trait UserReadParam<R>`, plus a downstream impl
/// that hands a **narrow** repo a **wide** read set. Expected to **compile**,
/// and that is the finding: this shape is forgeable in one line.
///
/// It does not collide with the blanket impl, because the blanket at
/// `R = DeclaredEmailAndName` yields `Repo<Domain, DeclaredEmailAndName, M>`,
/// which is never `Repo<Domain, DeclaredEmailOnly, ()>`. Coherence is correct
/// and the guarantee is still gone. This is RK-009 on a trait that no document
/// lists as needing a seal.
#[cfg(feature = "f1-forge-parameterised-trait")]
pub mod f1 {
    use super::{DeclaredEmailAndName, DeclaredEmailOnly, Domain, ReadName};
    use fw::Repo;
    use verum::Has;

    pub trait UserReadParam<R> {
        fn name<'a, I>(&self, d: &'a Domain) -> &'a str
        where
            R: Has<ReadName, I>;
    }

    impl<R, M> UserReadParam<R> for Repo<Domain, R, M> {
        fn name<'a, I>(&self, d: &'a Domain) -> &'a str
        where
            R: Has<ReadName, I>,
        {
            &d.name
        }
    }

    // One line, written by a downstream crate. No `unsafe`, no macro.
    impl UserReadParam<DeclaredEmailAndName> for Repo<Domain, DeclaredEmailOnly, ()> {
        fn name<'a, I>(&self, d: &'a Domain) -> &'a str
        where
            DeclaredEmailAndName: Has<ReadName, I>,
        {
            &d.name
        }
    }

    /// The repo declared `email` only, and this reads `name`.
    pub fn forged(d: &Domain) -> String {
        let r: Repo<Domain, DeclaredEmailOnly, ()> = Repo::new();
        UserReadParam::<DeclaredEmailAndName>::name::<_>(&r, d).to_owned()
    }
}

/// F2 — the same forge against the **unparameterised** trait. Expected to
/// **fail** with `E0119`.
///
/// F1 is F2's rule-14 control, and the pair is the whole finding: dropping the
/// type parameter is what closes the hole, because now every impl target is
/// `Repo<Domain, _, _>` and a concrete one overlaps the blanket.
#[cfg(feature = "f2-forge-associated-trait")]
impl UserRead for Repo<Domain, DeclaredEmailOnly, ()> {
    fn email<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadEmail, I>,
    {
        &d.email
    }
    fn name<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadName, I>,
    {
        &d.name
    }
    fn secret<'a, I>(&self, d: &'a Domain) -> &'a str
    where
        Self::Set: Has<ReadSecret, I>,
    {
        &d.secret
    }
}

/// G2 — a downstream crate re-pointing `ReadSet::Set` to a wider set, which
/// would defeat F2's fix by lying about what `Self::Set` is. Expected to
/// **fail** with `E0117`: both the trait and the type are foreign here.
///
/// This is what makes the associated-type form load-bearing rather than
/// decorative — the seal is the orphan rule, not a `Sealed` supertrait.
#[cfg(feature = "g2-repoint-readset")]
impl ReadSet for Repo<Domain, DeclaredEmailOnly, ()> {
    type Set = DeclaredEmailAndName;
}

// ---------------------------------------------------------------------------
// (c) Who supplies `R`?
// ---------------------------------------------------------------------------

/// G1 — the caller constructs its own `Repo` and picks the read set. Expected to
/// **compile**, and that is the finding.
///
/// The bound constrains `R`. It does not constrain who chooses `R`. Every
/// enforcement claim about these getters is conditional on `Repo` being
/// unreachable except through `Ctx` — a precondition the first version of this
/// spike did not state. `docs/dev/code/review-knowledge.md` RK-017 is the same
/// shape, recorded one PR earlier for `+ Send`.
pub fn g1_caller_picks_read_set(d: &Domain) -> String {
    let r: Repo<Domain, DeclaredEmailAndName, ()> = Repo::new();
    r.name(d).to_owned()
}

// ---------------------------------------------------------------------------
// (d) The Domain-side shape, and what the turbofish actually costs.
// ---------------------------------------------------------------------------

/// Shape D. The trait is generic over the read set because `Domain` cannot be:
/// making `Domain` carry `R` *is* `Projection` under another name.
pub trait DomainRead<R> {
    fn d_email<'a, I>(&'a self) -> &'a str
    where
        R: Has<ReadEmail, I>;

    fn d_name<'a, I>(&'a self) -> &'a str
    where
        R: Has<ReadName, I>;
}

impl<R> DomainRead<R> for Domain {
    fn d_email<'a, I>(&'a self) -> &'a str
    where
        R: Has<ReadEmail, I>,
    {
        &self.email
    }

    fn d_name<'a, I>(&'a self) -> &'a str
    where
        R: Has<ReadName, I>,
    {
        &self.name
    }
}

/// D1 — `user.email()`, the shape the design would prefer. Expected to **fail**
/// with `E0283`: nothing at the call site determines `R`.
#[cfg(feature = "d1-domain-getter-infer")]
pub fn d1_domain_getter_infer(d: &Domain) -> String {
    d.d_email().to_owned()
}

/// D2 — the same call naming **only `R`**, with the index left to inference.
/// Expected to **compile**.
///
/// This is D1's rule-14 control: the identical call succeeds the moment `R` is
/// determined, so D1's `E0283` is about `R` and not about the method.
///
/// It also corrects what the first version of this spike concluded. That draft
/// wrote the index by hand, saw it work, and inferred that callers must know
/// each field's *position* — so that adding one field to `reads` would renumber
/// every call site. D2 and D2b together show that is false: `_` suffices, and
/// the position never appears. Only `R` has to be named.
pub fn d2_domain_getter_names_r_only(d: &Domain) -> String {
    DomainRead::<DeclaredEmailOnly>::d_email::<_>(d).to_owned()
}

/// D2b — the same, for a field at position 1 rather than 0, still with `_`.
/// Expected to **compile**. Together with D2 this is the renumbering claim's
/// counter-example: one call form works at every position.
pub fn d2b_domain_getter_at_depth(d: &Domain) -> String {
    DomainRead::<DeclaredEmailAndName>::d_name::<_>(d).to_owned()
}

/// D2c — naming `R` explicitly still rejects an undeclared read. Expected to
/// **fail** with `E0277`.
///
/// D2/D2b are the control: the turbofish is an ergonomic escape from inference,
/// not from the capability check.
#[cfg(feature = "d2c-turbofish-undeclared")]
pub fn d2c_turbofish_undeclared(d: &Domain) -> String {
    DomainRead::<DeclaredEmailOnly>::d_name::<_>(d).to_owned()
}

/// D3 — the repository passed as a witness, so `R` comes from an argument.
/// Expected to **compile**.
pub fn d3_domain_getter_with_witness(
    r: &Repo<Domain, DeclaredEmailOnly, ()>,
    d: &Domain,
) -> String {
    r.email(d).to_owned()
}

// ---------------------------------------------------------------------------
// (e) What breaks — the view conversion.
// ---------------------------------------------------------------------------

pub struct View {
    pub email: String,
}

impl Domain {
    /// What `handler-rules.md`'s canonical example calls (`user.email()`). Its
    /// existence is itself a finding: see P2.
    pub fn plain_email(&self) -> &str {
        &self.email
    }
}

/// V2 — the control. A `From` impl reading through a **plain** accessor
/// compiles, because nothing checks anything. Expected to **compile**.
impl From<&Domain> for View {
    fn from(d: &Domain) -> Self {
        Self {
            email: d.plain_email().to_owned(),
        }
    }
}

/// V1 — the same conversion through a **checked** getter. Expected to **fail**
/// with `E0283`: a `From` impl has no `R` and no repository, so there is nowhere
/// for the capability to come from.
///
/// This is the cost `read-contract.md` records for `Projection` ("response
/// conversion gets fiddly") reappearing in the design that was supposed to
/// avoid it.
#[cfg(feature = "v1-view-from-checked-getter")]
impl From<&Domain> for ViewChecked {
    fn from(d: &Domain) -> Self {
        Self {
            email: d.d_email().to_owned(),
        }
    }
}

#[cfg(feature = "v1-view-from-checked-getter")]
pub struct ViewChecked {
    pub email: String,
}

// ---------------------------------------------------------------------------
// (f) What reads with no capability at all — the honesty half.
// ---------------------------------------------------------------------------

/// P2 — a free function reading a field with no capability in sight. Expected to
/// **compile**, which is the finding: `handler-rules.md` Rule 2 relies on free
/// associated functions being pure, and purity is a convention, not a check.
///
/// Note this is not about `plain_email` being sloppy: **something** must be able
/// to read the field, or the repository implementation could not build a row and
/// no view could ever be produced. The question is whether `reads` constrains it.
pub fn p2_free_function_reads(d: &Domain) -> String {
    format!("audit: {}", d.plain_email())
}

/// P1 — `Debug` on the domain, printing every field including undeclared ones.
///
/// Hand-written here rather than derived, because `verum-macros` emits nothing
/// yet; the derive's output would be this, and the point is what it can see.
impl std::fmt::Debug for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Domain")
            .field("email", &self.email)
            .field("name", &self.name)
            .field("secret", &self.secret)
            .finish()
    }
}

/// P1 — reading every field with no capability. Expected to **compile**.
pub fn p1_debug_reads_every_field(d: &Domain) -> String {
    format!("{d:?}")
}

// ---------------------------------------------------------------------------
// (g) Can a `Projection`'s `Debug` narrow to the declared set?
// ---------------------------------------------------------------------------
//
// The first version of this spike said no, on the grounds that "a derive sees
// tokens and `F` is a type parameter". That reasoning is wrong, and P4 is the
// counter-example. The derive does not have to enumerate `F` — it emits one
// impl **per field of the Domain**, which it can see, and a fixed recursive walk
// resolves `F` at monomorphisation.
//
// P4 is a `bin`, not a `#[test]`: what has to be observed is the *output*, and
// `run.sh` asserts the printed text.

/// A projection, exactly as `read-contract.md` describes it: the value's *type*
/// records the read set.
pub struct Projection<D, F> {
    inner: D,
    _f: PhantomData<fn() -> F>,
}

impl<F> Projection<Domain, F> {
    pub fn new(inner: Domain) -> Self {
        Self {
            inner,
            _f: PhantomData,
        }
    }

    /// The getter shape `read-contract.md:50` specifies.
    pub fn email<'a, I>(&'a self) -> &'a str
    where
        F: Has<ReadEmail, I>,
    {
        &self.inner.email
    }
}

/// One impl per **field of the Domain** — what a derive can see and emit.
pub trait DebugOneField<Elem> {
    fn fmt_one(&self, f: &mut std::fmt::DebugStruct<'_, '_>);
}

impl DebugOneField<ReadEmail> for Domain {
    fn fmt_one(&self, f: &mut std::fmt::DebugStruct<'_, '_>) {
        f.field("email", &self.email);
    }
}

impl DebugOneField<ReadName> for Domain {
    fn fmt_one(&self, f: &mut std::fmt::DebugStruct<'_, '_>) {
        f.field("name", &self.name);
    }
}

impl DebugOneField<ReadSecret> for Domain {
    fn fmt_one(&self, f: &mut std::fmt::DebugStruct<'_, '_>) {
        f.field("secret", &self.secret);
    }
}

/// A fixed recursive walk over the cons list, written once in the framework.
/// **Nothing here enumerates `F`** — monomorphisation does.
pub trait DebugSet<D> {
    fn fmt_set(d: &D, f: &mut std::fmt::DebugStruct<'_, '_>);
}

impl<D> DebugSet<D> for () {
    fn fmt_set(_: &D, _: &mut std::fmt::DebugStruct<'_, '_>) {}
}

impl<D, H, T> DebugSet<D> for (H, T)
where
    D: DebugOneField<H>,
    T: DebugSet<D>,
{
    fn fmt_set(d: &D, f: &mut std::fmt::DebugStruct<'_, '_>) {
        <D as DebugOneField<H>>::fmt_one(d, f);
        <T as DebugSet<D>>::fmt_set(d, f);
    }
}

impl<F: DebugSet<Domain>> std::fmt::Debug for Projection<Domain, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Projection");
        <F as DebugSet<Domain>>::fmt_set(&self.inner, &mut s);
        s.finish()
    }
}

/// P3 — the projection enforces reads through its getter, as the repo-side one
/// does. Expected to **compile**.
pub fn p3_projection_getter(p: &Projection<Domain, DeclaredEmailOnly>) -> String {
    p.email().to_owned()
}

/// P3b — and it rejects an undeclared read. Expected to **fail** with `E0277`.
///
/// P3 is the control. Without this half, "the projection also enforces" would be
/// a pass-only claim.
#[cfg(feature = "p3b-projection-undeclared")]
pub fn p3b_projection_undeclared(p: &Projection<Domain, DeclaredEmailOnly>) -> String {
    // `Projection` has no `name` getter, so reach for the set directly.
    fn need_name<F: Has<ReadName, I>, I>(_: &Projection<Domain, F>) {}
    need_name(p);
    p.email().to_owned()
}

// §9-13. A `pass` row asserts `Finished`, so a control that is *deleted* rather
// than broken would leave its row green having compiled nothing. These pin the
// baseline controls by name; deleting one is E0425 at the anchor.
const _: () = {
    let _ = e2_declared_read;
    let _ = e2b_declared_read_at_depth;
    let _ = d2_domain_getter_names_r_only;
    let _ = d2b_domain_getter_at_depth;
    let _ = d3_domain_getter_with_witness;
    let _ = p2_free_function_reads;
    let _ = p1_debug_reads_every_field;
    let _ = p3_projection_getter;
    let _ = g1_caller_picks_read_set;
};
