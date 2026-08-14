//! Only `Here` and `There<I>` may occupy an index position.
//!
//! Membership is already sealed, so this is defence in depth — but a forged
//! index would otherwise surface as a confusing membership failure rather than
//! a direct one.

struct MyIdx;

impl verum::Index for MyIdx {}

fn main() {}
