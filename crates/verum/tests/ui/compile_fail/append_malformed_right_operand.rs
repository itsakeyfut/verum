//! The right operand is the one whose diagnostic is broken, and it is the shape M8
//! emits.
//!
//! `append_malformed_operand.rs` malforms the left operand and reports it correctly.
//! Malform the *right* one under an `::Out` projection and the reported `Self`
//! collapses from the list the reader wrote to `()`:
//!
//! ```text
//! error[E0277]: `()` and `(C, D)` cannot be concatenated
//! ```
//!
//! `(A, (B, ()))` never appears, because the failure genuinely terminates at
//! `impl<B: ConsList> Append<B> for ()` — and `do_not_recommend` cannot help there,
//! since that impl *is* where the obligation ends. This is RK-006's exact failure
//! occurring **with** the mitigation in place, which nothing had recorded.
//!
//! Pinned rather than fixed: the span still points at the real types, so the error
//! is actionable, and no arrangement of `on_unimplemented` can reach past the
//! terminus. `M8` composes `<E::Mutates as Append<CondMutates>>::Out`, so this is
//! the shape a user will actually hit when a `when` block's set is malformed.

pub struct A;
pub struct B;
pub struct C;
pub struct D;

/// Well-formed left, flat-tuple right.
type Right = (C, D);

fn main() {
    let _: core::marker::PhantomData<<(A, (B, ())) as verum::Append<Right>>::Out> =
        core::marker::PhantomData;
}
