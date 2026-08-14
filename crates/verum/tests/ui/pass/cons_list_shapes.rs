//! Every set size is constructible, and the index markers nest.
//!
//! Paired with `flat_tuple_is_not_a_cons_list.rs`: without this, an
//! implementation that rejected *every* shape would still show a green
//! compile_fail suite (docs/rules/test.md §2).

use verum::{ConsList, Here, Index, There};

struct A;
struct B;
struct C;

fn accepts_set<L: ConsList>() {}
fn accepts_index<I: Index>() {}

fn main() {
    accepts_set::<()>();
    accepts_set::<(A, ())>();
    accepts_set::<(A, (B, ()))>();
    accepts_set::<(A, (B, (C, ())))>();

    accepts_index::<Here>();
    accepts_index::<There<Here>>();
    accepts_index::<There<There<Here>>>();
}
