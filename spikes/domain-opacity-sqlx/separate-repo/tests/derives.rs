//! #44's foreign-crate half. Ledger paths 26 and 28.
//!
//! Both routes are stated as crossing a crate boundary, and both are measured from
//! here rather than from `app`, because "the same crate can do it" is a weaker
//! claim than the paths make. Neither test names a field: they are private, and the
//! generated impls read them from inside `app`'s own module. Field privacy is not
//! the lever — the derive is.

/// P43 — ledger path 26. A derive the user attached in the position `#[domain]`
/// cannot see, under the shape-preserving form.
///
/// Three routes in one test, because they are one finding and each is cheap:
/// `from_str` invents a Domain with attacker-chosen values, `clone` takes an owned
/// copy of it, and `mem::take` reinitialises through a `&mut` alone. None of them
/// touches `Repr`, so none of them is path 21.
#[cfg(feature = "p43-forge-via-derive")]
#[test]
fn a_foreign_crate_should_forge_a_domain_through_a_user_attached_derive() {
    let mut forged: app::derives::SameModuleAccount =
        serde_json::from_str(r#"{"email":"attacker@example.com"}"#).expect("attacker-authored json");
    assert_eq!(forged.email(), "attacker@example.com");

    let stolen = forged.clone();
    assert_eq!(stolen.email(), "attacker@example.com");

    // `mem::take` needs only `&mut` and `Default`: it puts a Domain nobody built
    // where a loaded one was. This is the route the path-2 comparison table said
    // could "only install a value some `find` returned".
    let taken = std::mem::take(&mut forged);
    assert_eq!(taken.email(), "attacker@example.com");
    assert_eq!(forged.email(), "");

    // run.sh needles on this line rather than on the test count, for the reason
    // `confined.rs` records: `ok. N passed` counts tests, so an emptied body stays
    // green. A value only the forgery can produce cannot be emptied away.
    println!("VERUM_P43_FORGED={}", stolen.email());
}

/// P45 — ledger path 28. The whitelist cannot see through the alias, and the
/// mutation needs no `&mut`.
///
/// `Order` is built through its legitimate route, so this is not a forged value:
/// it is a **correctly loaded** Domain whose contents a foreign crate changes
/// through a shared reference. That is what puts it inside a GET's scope, where
/// `capability-system.md`'s read-only guarantee is supposed to hold.
#[cfg(feature = "p45-mutate-through-shared-ref")]
#[test]
fn a_shared_reference_should_reach_inside_the_field_type() {
    let order = app::derives::Order::load();
    assert_eq!(order.audit().borrow().len(), 0);

    // No `&mut order` anywhere, and no capability. `&self` is the whole cost.
    let readonly: &app::derives::Order = &order;
    readonly.audit().borrow_mut().push("written by a GET".to_owned());

    assert_eq!(order.audit().borrow().len(), 1);
    println!("VERUM_P45_AUDIT={}", order.audit().borrow()[0]);
}
