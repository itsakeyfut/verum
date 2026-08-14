//! The sealed trait is still usable as a bound by downstream code.
//!
//! Paired with the compile_fail cases so that "everything fails to compile"
//! cannot masquerade as a passing suite (docs/rules/test.md §2).

struct Order;

fn requires_domain_access<E>()
where
    E: verum::Includes<Order>,
{
}

fn main() {}
