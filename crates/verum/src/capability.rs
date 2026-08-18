//! Capability handles.
//!
//! A capability is never a value the user can name or construct — it is expressed
//! through the type parameters of the handle `Ctx` hands out
//! (`docs/rules/api-surface.md`). This module holds that handle.

use core::marker::PhantomData;

/// The marker for parameters that exist only at the type level.
///
/// Factored out because `clippy::type_complexity` rejects the inline form, and
/// `#[allow]`-ing a lint on a field of the crate's own public type is the wrong
/// side of that trade — the alias also gives the shape one place to be explained.
///
/// `fn() -> T` rather than `T`: the handle owns no value of these types, so this
/// form implies neither `Drop` nor any auto-trait obligation on them, and stays
/// covariant.
type TypeLevelOnly<T> = PhantomData<fn() -> T>;

/// The capability handle for one domain — what `ctx.users()` returns.
///
/// `D` is the domain, `R` its readable field set and `M` its mutable field set,
/// both cons lists. Field-level checking happens in the where clause of the
/// extension trait's methods, never on this type directly
/// ([ADR-0002](https://github.com/itsakeyfut/verum/blob/main/docs/adr/0002-ctxusers-exposes-the-endpoint-as-owner.md)).
///
/// # `'req` is the point of this type
///
/// `Ctx<'req, E>` is deliberately not `'static`, so a capability cannot leave the
/// request. Before #39 the handle carried **no lifetime**, so the `Ctx` was
/// contained and everything it produced was not — a handle taken from a correctly
/// scoped `Ctx`, moved out, and used after the response had been sent mutated the
/// store 150 ms later. That was measured at *run time*, not argued:
/// `spikes/ctx-lifetime-rpitit`, `e1_leaked_handle_mutates_after_the_request_scope_ended`.
///
/// What made it easy to miss is that field-granular checking survives the escape
/// — an escaped handle still cannot touch an undeclared field. **It is scope
/// escape, not capability forgery**, and every one of #14's acceptance criteria
/// passed while it was open.
///
/// `'req` is therefore load-bearing rather than decorative, and
/// `tests/ui/compile_fail/repo_handle_cannot_outlive_its_request.rs` is what keeps
/// it that way.
///
/// # Why there is no constructor
///
/// A capability value with a public constructor is a capability anyone can mint.
/// The constructor is gated on the sealed runtime token and arrives with it
/// (ADR-0006, `status: proposed`). Until then this type is declarable and
/// bindable but not constructible outside the crate, which is the correct order:
/// the *shape* is what becomes a breaking change after M2, and the shape is what
/// #39 settled.
pub struct Repo<'req, D, R, M> {
    /// Binds the handle to the request that granted it (#39, ADR-0005).
    ///
    /// Which concrete field ultimately carries the lifetime — a `&'req Runtime`,
    /// or an owned handle plus this marker — is **#40 / ADR-0006's** to decide,
    /// and ADR-0005 records that #39 and #40 cannot be settled independently.
    /// Both candidates were measured (`RepoLt`, `RepoPhantom`; probes E2/E3/E4a/E4b)
    /// and **both carry `'req` as the first parameter**, so the declared shape is
    /// the same either way. Fixing the shape now costs one parameter; fixing it
    /// after M2 is a breaking change.
    ///
    /// Covariant in `'req`, matching a `&'req` field. Invariance would be a
    /// *brand*, which is a different mechanism and a separate question.
    _req: PhantomData<&'req ()>,
    /// The domain and its two field sets. See [`TypeLevelOnly`] for why the
    /// marker is written the way it is.
    _model: TypeLevelOnly<(D, R, M)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct User;

    /// A `Repo` **is** `Send` — hyper's multi-thread runtime requires it — and is
    /// `Send` *unconditionally*, which is the half `fn() -> T` buys.
    ///
    /// The name says only `send` on purpose. The other half of the design
    /// ("**not** `'static`") cannot be *failed* inside a unit test, so it lives in
    /// `tests/ui/compile_fail/repo_handle_cannot_outlive_its_request.rs`; an earlier
    /// name claimed both and checked one.
    ///
    /// ⚠️ Once #40 gives `Repo` a real field this becomes **conditional** — a
    /// `&'req Runtime` is `Send` only if `Runtime: Sync`. Auto-trait impls are
    /// semver-visible, so that is one public property the deferral to #40 does not
    /// in fact cover.
    #[test]
    fn repo_should_be_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Repo<'_, User, (), ()>>();

        // `*const u8` is not `Send`. This line is what makes the test check the
        // *reason* rather than the outcome: with `_model: PhantomData<(D, R, M)>`
        // instead of `PhantomData<fn() -> (D, R, M)>` it stops compiling
        // (`E0277`), whereas the `User` case above stays green either way. Review
        // measured that the mutation was otherwise invisible to the whole suite.
        assert_send::<Repo<'_, *const u8, (), ()>>();
    }
}
