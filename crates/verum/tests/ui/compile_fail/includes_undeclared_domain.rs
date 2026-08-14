//! Reaching a domain the contract does not declare.
//!
//! This is the shape almost every real Verum error takes — a bound that is not
//! satisfied at a *use* site, not a hand-written impl. The seal's own
//! diagnostic cannot reach here (the unsatisfied bound is `Includes`, not
//! `Sealed`), which is why `Includes` carries its own.

struct Order;
struct GetUser;

fn reaches_order<E: verum::Includes<Order>>() {}

fn main() {
    reaches_order::<GetUser>();
}
