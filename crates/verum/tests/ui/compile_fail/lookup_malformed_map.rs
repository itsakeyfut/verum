//! A malformed map does not resolve, and a forged index cannot reach `Lookup`.
//!
//! This fixture exists because of a mutation-testing gap, not a bug report.
//! `Lookup`'s `T: ConsList` and `I: Index` bounds were behaving correctly —
//! measured — but **nothing failed when they were removed**, which means the suite
//! could not tell the difference between the bounds being there and being deleted.
//! An untested bound is dead weight the next person will helpfully "simplify".
//!
//! A map whose tail is not a cons list. `Lookup` walks pairs, so a broken spine must
//! stop it rather than returning whatever it happened to reach. Pinned at depth,
//! because the head impl and the recursive impl carry the bound separately.
//!
//! The forged-index case that used to live here now has its own file,
//! `lookup_forged_index_message_is_misleading.rs` — it pins a deliberately
//! misleading message, and sharing a `.stderr` with a case T-M2-09 will improve
//! would have let that pinned wording be silently regenerated away.

pub struct K1;
pub struct K2;
pub struct V1;
pub struct V2;
pub struct NotAConsList;

fn lookup<Map, K, I>()
where
    Map: verum::Lookup<K, I>,
{
}

fn main() {
    // The spine breaks after the first pair.
    lookup::<((K1, V1), ((K2, V2), NotAConsList)), K2, _>();
}
