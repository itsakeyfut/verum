//! Both operations resolve downstream, through the public re-export, with the
//! seals in place.
//!
//! The unit tests in `typelevel.rs` prove the same arithmetic in-crate, where the
//! seals are satisfiable trivially. This file is the one that proves a *user* can
//! use them: the seal is unnameable here, and `Lookup`'s index is inferred rather
//! than written.
//!
//! `pass` fixtures are not decoration. Without one, an implementation where
//! everything failed to compile would still show a green `compile_fail` suite.

pub struct A;
pub struct B;
pub struct C;

trait IsSame<T> {}
impl<T> IsSame<T> for T {}

fn assert_same<X: IsSame<Y>, Y>() {}

fn lookup_is<Map, K, I, Expected>()
where
    Map: verum::Lookup<K, I>,
    <Map as verum::Lookup<K, I>>::Out: IsSame<Expected>,
{
}

/// Any `Append` output can be used where a well-formed set is required, with no
/// bound restated at the call site — that is the `type Out: ConsList` guarantee.
fn requires_well_formed<L: verum::ConsList>() {}

fn main() {
    // Concatenation, including both empty cases.
    assert_same::<<() as verum::Append<(A, ())>>::Out, (A, ())>();
    assert_same::<<(A, ()) as verum::Append<()>>::Out, (A, ())>();
    assert_same::<<(A, ()) as verum::Append<(B, ())>>::Out, (A, (B, ()))>();
    assert_same::<<(A, (B, ())) as verum::Append<(C, ())>>::Out, (A, (B, (C, ())))>();

    requires_well_formed::<<(A, (B, ())) as verum::Append<(C, ())>>::Out>();

    // Lookup at head, middle and tail, with the index inferred every time.
    struct K1;
    struct K2;
    struct K3;
    type Map = ((K1, A), ((K2, B), ((K3, C), ())));

    lookup_is::<Map, K1, _, A>();
    lookup_is::<Map, K2, _, B>();
    lookup_is::<Map, K3, _, C>();

    // The composed result is itself a valid operand — this is the shape M8 needs,
    // where a `when` scope's set is appended and then searched.
    type Composed = <(A, ()) as verum::Append<(B, ())>>::Out;
    fn is_member<Set, T, I>()
    where
        Set: verum::Has<T, I>,
    {
    }
    is_member::<Composed, A, _>();
    is_member::<Composed, B, _>();
}
