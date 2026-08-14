//! The failure mode that motivates the cons list representation.
//!
//! `(Order, Item)` reads as head `Order`, tail `Item`, so a flat tuple
//! *appears* to work at exactly two elements and breaks at three. Without
//! `ConsList` this shape compiles and the mistake survives to runtime shape
//! errors much later. See docs/rules/type-level.md §1 (RK-002).

struct Order;
struct Item;

fn requires_effect_set<L: verum::ConsList>() {}

fn main() {
    requires_effect_set::<(Order, Item)>();
}
