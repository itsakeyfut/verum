//! A wrong index reports "no entry for key K", where K *is* a key. Pinned on purpose.
//!
//! `Index`'s own `on_unimplemented` never fires, because the bound that fails is
//! `Lookup`'s — the same dead end measured for `ConsList`-through-`Has` in T-M0-08.
//! So the reader is told the map lacks a key it actually has, when the real fault is
//! the index.
//!
//! Kept rather than fixed, for the reason `has_malformed_set_message_is_misleading.rs`
//! is kept: a limitation nobody can see is a limitation nobody fixes. The intent lives
//! in the **file name**, because that is the only thing visible in a
//! `TRYBUILD=overwrite` diff.
//!
//! **It lives in its own file for exactly that reason.** It used to share a `.stderr`
//! with `lookup_malformed_map.rs`, whose message T-M2-09 is expected to improve — so
//! the regeneration would have silently rewritten this deliberately-pinned wording
//! under a filename that gave no hint a pinned limitation was being overwritten.
//!
//! Also note this is *not* limited to forged indices: writing any wrong-but-valid
//! index for a key that exists produces the same misleading text. Indices are filled
//! by inference in practice, which is what keeps it rare rather than impossible.

pub struct K1;
pub struct K2;
pub struct V1;
pub struct V2;
pub struct NotAnIndex;

fn lookup<Map, K, I>()
where
    Map: verum::Lookup<K, I>,
{
}

fn main() {
    // A well-formed map. `K2` is genuinely a key. Only the index is wrong.
    lookup::<((K1, V1), ((K2, V2), ())), K2, verum::There<NotAnIndex>>();
}
