# #41 — are the derive-facing seals forgeable once M2 exposes them?

**Yes, and the ledger's own re-verification procedure is green on the tree where the
forgery compiles.** That second half is the finding that matters.

Reproduce: `bash run.sh` → `5 as specified, 0 unexpected`.

## Why this is a separate workspace

The question needs a genuinely **downstream** crate, and the probe deliberately
exposes a module that `crates/verum` keeps `pub(crate)`. Doing that inside the real
crate would mean committing the exposure in order to measure it.

## Probes

| # | What it does | Expected |
|---|---|---|
| **S1** | the ledger's recorded procedure, verbatim: `impl Includes<undeclared>` with **no** seal impl | fail `E0277` |
| **S2** | the attacker writes the seal impl **too**, and *uses* the forged bound | **pass = the defect** |
| **S3** | the same attack against the blanket shape | fail — and with Verum's own wording |
| **S4** | #41's *stated* reason, on its own terms: does coherence reject a competing impl? | **pass = the reason is wrong** |
| **S5** | the blanket shape still serves a declared domain (§9-14 control for S3) | pass |

## The verdict

### The seal stops working, it does not merely weaken

`unverified-boundaries.md:152` says exposing `__private` at M2 is "the moment the seal
weakens". S2 shows it is the moment the seal **stops working**: two undeclared domains
pass the Architecture Contract from an ordinary downstream crate, no `unsafe`, no
build script.

This is structural, not an implementation defect. **Proc-macro output is
syntactically indistinguishable from hand-written code**, so an obligation a derive
can discharge downstream, a human can discharge downstream. Sealing on `(Self, D)`
rather than `Self` is still correct and still insufficient.

### The procedure was written against the wrong attacker

The ledger prescribes: *"with the derive emitting one domain's seal, confirm
`impl Includes<未宣言>` is E0277"*. That is S1, and S1 is **green on the same tree
where S2 compiles.** The procedure imagines an attacker who does not write the seal;
once the seal is public, the attacker writes the seal too.

**This is the fourth instance of one class** — #6 (type parameters), #8 (recursion),
#9 (a bound on a parameter not in `Self`), and now the *verification procedure*
rather than the implementation. `api-surface.md` §2's rule ("a closure's justification
must cover every impl position") has to apply to procedures as well as to seals.

### The blanket direction works, and not for the reason #41 gives

#41 proposes `impl<E, D, I> Includes<D, I> for E where E: Endpoint, E::Domains: Has<D, I>`
and argues that nothing is left to forge because coherence would reject a competing
impl. **S4 refutes that**: rustc judges the blanket and a specific impl **disjoint**
precisely when the blanket's obligation is unsatisfiable — which is exactly the
undeclared domain. So coherence admits the one impl an attacker wants. CLAUDE.md
already records this from T-M0-08: *coherence permits only the harmful side.*

What the blanket shape actually buys is one step earlier:

> **With nothing emitted per domain, the derive never names the seal — so
> `SealedIncludes` moves from *derive-facing* to *structural*, and structural seals
> stay `pub(crate)` forever.**

S3 is the consequence: the attacker cannot write the seal impl because the module is
still private, and the blanket seal does not apply for an undeclared domain. `E0277`.

### The diagnostics get better, not worse

Measured side by side:

```
S1 (per-domain):  error[E0277]: `GetUser` cannot implement a sealed Verum trait
                                 ^^^^^^^ this trait is sealed

S3 (blanket):     error[E0277]: `GetUser` does not declare the domain `Secrets`
                                 ^^^^^^^ reaching `Secrets` requires declaring it
                  = note: either add `Secrets` to this endpoint's declared domains, …
                  note: required by a bound in `Includes`
```

The blanket form names the **domain** and says what to do; the per-domain form only
says "sealed". Two attributes are load-bearing for that, and both were measured:

* `#[diagnostic::on_unimplemented]` goes on the **seal**, not on the public trait —
  with a blanket impl the failing obligation is the seal's, so a message on `Includes`
  never fires.
* `#[diagnostic::do_not_recommend]` goes on **both** blanket impls. Without it rustc
  drills through to the raw `Has<Secrets, Here>` bound and the wording is lost
  entirely.

### What is left, and cannot be closed by types

`Endpoint` itself. The derive emits `impl Endpoint for X`, so `SealedEndpoint` must be
nameable downstream under **either** shape — `fw/src/lib.rs` declares it in
`derive_facing` for that reason, and the probes rely on it. A forged `Endpoint` can
declare any `Domains`, so the blanket `Includes` faithfully reports a set the attacker
chose.

The exposure still drops from *N domain relations* to *one `Endpoint` declaration*,
which is #41's other claim and is correct. Per ARK-005 the residual value of type
enforcement is making relaxation visible, so what remains is an inventory check:
compare the declarations the macro emitted against the impls that exist.

## A detail #41 did not record — and the wrong conclusion drawn from it

`pub use sealed::derive_facing as __private;` on a `pub(crate)` **module** is
**`E0365`** — *"only public within the crate, and cannot be re-exported outside"*. That
part reproduces.

**The conclusion "so M2 must declare the module `pub`" is false**, and review measured
it: re-export the **traits** item-wise instead —

```rust,ignore   // fragment, not a complete item
#[doc(hidden)]
pub mod __private {
    pub use crate::sealed::derive_facing::SealedIncludes;
}
```

— and `derive_facing` stays `pub(crate)`, `private` stays `E0603`, and it compiles.
That is **strictly narrower** than `pub mod derive_facing`: only the named traits
escape, so a seal added to `derive_facing` later is *not* automatically exposed.

So the option the specs implicitly assumed **is** available, and it is the better one.
This spike declares the module `pub` because that is the widest shape and therefore the
right one to attack — but the earlier version of this section recorded the wide shape
as *forced*, which is the opposite of the finding. It also has a consequence for the
guard in `crates/verum/src/sealed.rs`: under the item-wise form the escape set is
decided by the `__private` re-export list, not by `derive_facing`'s module body, so a
scan of that body is looking in the wrong place. **#60 decides which form to take**;
whichever it is, the guard has to scan the one that governs.

## Measured on

rustc 1.85.0, 2026-08-18. `EXPECTED_ROWS` in `run.sh` is the row count.
