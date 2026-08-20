//! The attacker's crate. Everything here is ordinary downstream Rust — no `unsafe`,
//! no build script, nothing a reviewer would flag on sight.

// ---------------------------------------------------------------------------
// S1 — the attack the ledger's recorded re-verification procedure imagines:
//      "confirm `impl Includes<undeclared>` is E0277".
//
// Expected to FAIL. It does, and that is the problem: it fails on the same tree
// where S2 compiles, so a green S1 says nothing.
// ---------------------------------------------------------------------------
#[cfg(feature = "s1-trait-only")]
mod s1 {
    pub struct GetUser;
    pub struct Order;
    impl fw::Includes<Order> for GetUser {}
}

// ---------------------------------------------------------------------------
// S2 — the attacker writes the seal too, because M2 made it public.
//
// Expected to COMPILE. Two undeclared domains pass the Architecture Contract.
// Proc-macro output is syntactically indistinguishable from hand-written code, so
// an obligation a derive can discharge downstream, a human can discharge downstream.
// ---------------------------------------------------------------------------
#[cfg(feature = "s2-seal-and-trait")]
mod s2 {
    pub struct GetUser;
    pub struct Order;
    pub struct Secrets;
    impl fw::derive_facing::SealedIncludes<Order> for GetUser {}
    impl fw::Includes<Order> for GetUser {}
    impl fw::derive_facing::SealedIncludes<Secrets> for GetUser {}
    impl fw::Includes<Secrets> for GetUser {}

    /// Not just declarable — *usable*. Without this the probe would only show the
    /// impls parse.
    pub fn use_it<E, D>()
    where
        E: fw::Includes<D>,
    {
    }
    pub fn forged() {
        use_it::<GetUser, Secrets>();
    }
}

// §9-13. Review gutted `mod s2` and the row stayed "as specified" — S2 is the
// probe the whole ADR rests on ("pass = the defect"), so a hollow S2 is the worst
// case in this spike. Gated on the same feature as the module, so re-pointing one
// and not the other stops compiling.
#[cfg(feature = "s2-seal-and-trait")]
const _: () = {
    let _ = s2::forged;
};

// ---------------------------------------------------------------------------
// S3 — the same attack against the blanket shape. There is no seal to name: it
// never left `fw`'s private module, because nothing is emitted per domain.
//
// Expected to FAIL with Verum's own wording.
// ---------------------------------------------------------------------------
#[cfg(feature = "s3-blanket-trait-only")]
mod s3 {
    pub struct GetUser;
    pub struct User;
    pub struct Secrets;
    // A legitimate endpoint: the seal it needs IS public, because the derive must
    // name it. Declares `User` only.
    impl fw::derive_facing::SealedEndpoint for GetUser {}
    impl fw::Endpoint for GetUser {
        type Domains = (User, ());
    }
    // The attack. There is no `SealedIncludes` to name — it never left `fw`.
    impl fw::Includes<Secrets, fw::Here> for GetUser {}
}

// ---------------------------------------------------------------------------
// S4 — the reason #41 gave, tested on its own terms.
//
// #41 says a blanket impl closes the hole because coherence rejects a competing
// impl. Expected to **COMPILE**, refuting that: rustc judges the two impls
// disjoint precisely when the blanket's obligation is unsatisfiable — which is
// exactly the undeclared domain. Recorded in CLAUDE.md as T-M0-08's lesson:
// coherence permits only the harmful side.
//
// So the blanket shape works, and not for the stated reason. What it actually buys
// is that the seal stops being derive-facing.
// ---------------------------------------------------------------------------
#[cfg(feature = "s4-blanket-coherence")]
mod s4 {
    pub struct GetUser;
    pub struct Unsealed;
    // A trait with a blanket impl over `E: Endpoint`, and a competing specific impl
    // for a type whose declared set does not contain the domain.
    impl fw::derive_facing::SealedEndpoint for GetUser {}
    impl fw::Endpoint for GetUser {
        type Domains = ();
    }
    // An UNSEALED trait with a blanket impl over `E: Endpoint`, plus a competing
    // specific impl. If coherence were what closes the hole, this would be E0119.
    pub trait Marker<D, I> {}
    impl<E, D, I> Marker<D, I> for E
    where
        E: fw::Endpoint,
        E::Domains: fw::Has<D, I>,
    {
    }
    impl Marker<Unsealed, fw::Here> for GetUser {}

    /// The witness. Without it the row is satisfied by an empty module, and S4 is
    /// the row that refutes the coherence reading.
    pub fn admitted<T: Marker<Unsealed, fw::Here>>() {}
}

#[cfg(feature = "s4-blanket-coherence")]
const _: () = {
    let _ = s4::admitted::<s4::GetUser>;
};

// ---------------------------------------------------------------------------
// S5 — the blanket shape still serves a legitimate declared domain.
// Without this, S3's rejection could just mean the shape is unusable.
// ---------------------------------------------------------------------------
#[cfg(feature = "s5-blanket-legitimate")]
mod s5 {
    pub struct User;
    pub struct GetUser;
    impl fw::derive_facing::SealedEndpoint for GetUser {}
    impl fw::Endpoint for GetUser {
        type Domains = (User, ());
    }
    fn needs<E, D, I>()
    where
        E: fw::Includes<D, I>,
    {
    }
    pub fn ok() {
        needs::<GetUser, User, fw::Here>();
    }
}

#[cfg(feature = "s5-blanket-legitimate")]
const _: () = {
    let _ = s5::ok;
};
