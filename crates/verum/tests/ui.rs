//! UI tests — Verum's primary test layer.
//!
//! Verum's claim is that wrong code does not compile, so these files *are* the
//! specification and the `.stderr` files are the specification of what the
//! failure says (`docs/rules/test.md` §1).
//!
//! `pass` is not decoration: without it, an implementation where *everything*
//! failed to compile would still show a green `compile_fail` suite.

/// trybuild reports a glob that matches nothing as a **pass**, and
/// `cargo test --workspace` does not run this target at all (`test = false`).
/// Together that means a directory rename or a bad merge can take the project's
/// primary test layer to zero coverage with CI fully green.
///
/// This is the same failure the boundary guard already defends against — see the
/// "empty scan" case in `check-api-boundary-test.sh`.
///
/// The floor is a **count, kept at the current count**. An earlier version left
/// it deliberately low so that adding cases need not touch it; at 3-of-6 that
/// meant half the suite — including both of the cases added with it — could be
/// deleted while CI stayed green (measured). Updating one number per new fixture
/// is the cheaper side of that trade.
fn assert_fixtures_present(dir: &str, floor: usize) {
    let found = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{dir} is unreadable: {e}"))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .count();
    assert!(
        found >= floor,
        "{dir} holds {found} case(s), expected at least {floor} — \
         trybuild reports an empty glob as a pass, so this would have been green"
    );
}

#[test]
fn contract_violations_should_not_compile() {
    assert_fixtures_present("tests/ui/compile_fail", 15);
    assert_fixtures_present("tests/ui/pass", 3);

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_fail/*.rs");
    t.pass("tests/ui/pass/*.rs");
}
