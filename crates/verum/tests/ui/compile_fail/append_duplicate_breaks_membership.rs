//! `Append` produces duplicates silently, and the damage shows up later at `Has`.
//!
//! This pins RK-011's **second** route, which had no fixture. The first route is a
//! hand-written duplicate (`has_duplicate_element.rs`); this one is worse, because
//! **the contract is valid**:
//!
//! ```ignore
//! emits = [OrderPlaced],
//! when(IsPaid) => { emits = [OrderPlaced] }
//! ```
//!
//! Composing those with `Append` yields `(OrderPlaced, (OrderPlaced, ()))`, the
//! index becomes ambiguous, and the user sees E0283 — an error about type
//! annotations, at a membership site, for a contract that reads perfectly.
//!
//! `Append` cannot prevent it. Deduplicating would mean testing each element of
//! one list for membership in the other and then branching on *absence*, which needs a
//! total membership decision — the catch-all impl collides (E0119) and the index witness
//! has nowhere to live (E0207). `Has` works only because it is a *partial* relation:
//! "absent" means "no impl", not a value you can branch on. (An earlier version of this
//! comment blamed `Subset` being forbidden, which is wrong twice over — `Subset` is banned for
//! cost, not possibility — though writing it naively fails with E0207 and needs the
//! witness threaded as an extra parameter.) So the
//! duplicate is produced deliberately (there is a unit test asserting exactly
//! that) and **dedup is unconditionally the macro's job** before `Append` runs.
//!
//! **This fixture will not change when M8 lands, and an earlier version of this
//! comment wrongly claimed it would.** M8's dedup is necessarily macro-side (type-level
//! dedup needs negative reasoning — see `Append`'s rustdoc), and this file hand-writes
//! the `Append` call, bypassing the derive entirely. Its `.stderr` is invariant under
//! anything M8 does.
//!
//! What it is actually for: pinning that layer 3 still catches the hazard for
//! hand-written and future generated sets, which is the same reason `ConsList` exists
//! alongside the derive's own folding.

pub struct OrderPlaced;

type TopLevel = (OrderPlaced, ());
type InsideWhen = (OrderPlaced, ());
type Composed = <TopLevel as verum::Append<InsideWhen>>::Out;

fn requires_member<Set, T, I>()
where
    Set: verum::Has<T, I>,
{
}

fn main() {
    requires_member::<Composed, OrderPlaced, _>();
}
