#!/usr/bin/env bash
#
# T-M1-03 / #15 — can capability-checked getters enforce `reads` without
# `Projection`?
#
# Every row is a compile whose expected outcome is declared up front. The verdict
# is what this script prints, not what anyone reasoned.
#
# NOT A CI GUARD, and deliberately not wired into any workflow — the same
# treatment as the other three spikes. It is separate from the root workspace so
# that a dozen deliberately-broken probes never reach `cargo check --workspace`.
# It DOES path-depend on `crates/verum`, because requirement 4 asks for the error
# text an undeclared read produces and only the real `Has` carries
# `on_unimplemented` and `do_not_recommend`.
#
# WHAT WOULD OTHERWISE CORRUPT THE RESULT — `docs/rules/test.md` §9
#   §9-1  Each rejection asserts its own expected message.
#   §9-3  `Finished`, never `Checking <crate>` — the latter is cache-dependent.
#   §9-4  `[lints.rust] unexpected_cfgs = "deny"` is set per package, so a
#         misspelled feature name in the SOURCE is an error rather than a probe
#         that silently compiles nothing. The command-line spelling is a
#         different hazard; see NEEDLES below.
#   §9-5  The baseline runs after a recursive `touch`, so a cached diagnostic
#         cannot replay as a pass.
#   §9-7  That touch recurses; a new subdirectory cannot disarm it.
#   §9-12 The toolchain is asserted, and the assertion is made non-vacuous by
#         checking installation first — see TOOLCHAIN below.
#   §9-13 Baseline controls are pinned by name in a `const _` block in
#         app/src/lib.rs, so deleting one is E0425 rather than a green row.
#   §9-14 Every rejection has a standing control that removes the cause it
#         names — the pairs are tabulated in README.md.
#
# NEEDLES ARE ERROR CODES, NOT PROSE.
#   The first version of this script used `does not contain`, which is a
#   substring of cargo's own `error: the package 'app' does not contain this
#   feature: <typo>`. A one-character typo in `--features` therefore produced
#   rc != 0 AND a needle match — the row printed `as specified` having compiled
#   nothing, and the suite exited 0. Measured, not reasoned. Error codes cannot
#   collide that way, which is what the sibling spikes already do.
#
# USAGE
#   bash run.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
export CARGO_TARGET_DIR="$PWD/target"

TOOLCHAIN="1.85.0"
CARGO=(cargo "+${TOOLCHAIN}")

pass=0
fail=0

echo "=== environment ==="
# §9-12. `rustc +$T --version` compared against `$T` is self-referential: rustup
# will silently DOWNLOAD a missing toolchain and the suite goes green on the
# wrong compiler (measured with 1.86.0). Check installation first so the
# assertion can actually fail.
if ! rustup toolchain list | grep -q "^${TOOLCHAIN}-"; then
    echo "FATAL: toolchain ${TOOLCHAIN} is not installed. Without this check" >&2
    echo "       rustup would fetch it and the measurement would be silent." >&2
    exit 1
fi
RUSTC_V="$(rustc "+${TOOLCHAIN}" --version)"
printf '  rustc        %s\n' "$RUSTC_V"
if [[ "$RUSTC_V" != rustc\ ${TOOLCHAIN}\ * ]]; then
    echo "FATAL: expected rustc ${TOOLCHAIN}, got ${RUSTC_V}." >&2
    exit 1
fi
# §9-12 again: assert the dependency edge rather than printing a literal.
if ! "${CARGO[@]}" metadata --format-version 1 --no-deps >/dev/null 2>&1; then
    echo "FATAL: the dependency tree cannot be resolved. The first probe's" >&2
    echo "       failure would otherwise be reported as a spike result." >&2
    exit 1
fi
printf '  layering     fw (framework: Repo, ReadSet) <- app (downstream: Domain)\n'
printf '  verum        path dependency on ../../crates/verum (the real sealed Has)\n'
echo

# §9-1. `needle` is what separates "rejected for the reason under test" from
# "rejected". In #13 two predicted error codes were wrong and this is what caught
# them; in #14 a probe produced the right code for the wrong reason.
probe() {
    local id="$1" expect="$2" needle="$3"
    shift 3

    local out rc=0
    out="$("${CARGO[@]}" "$@" 2>&1)" || rc=$?

    local got="pass"
    [[ $rc -ne 0 ]] && got="fail"

    local reason="ok"
    if ! grep -qF -- "$needle" <<<"$out"; then
        reason="MISSING(\"$needle\")"
    fi

    if [[ "$got" == "$expect" && "$reason" == "ok" ]]; then
        printf '  %-6s %-6s %-6s  %s\n' "$id" "$expect" "$got" "as specified"
        pass=$((pass + 1))
    else
        printf '  %-6s %-6s %-6s  %s\n' "$id" "$expect" "$got" "UNEXPECTED — $reason"
        fail=$((fail + 1))
        sed 's/^/       | /' <<<"$out" | grep -E '^\s+\| error' | head -4 || true
    fi
}

# §9-5 / §9-7. Without invalidating the cache first, cargo replays a stored
# diagnostic and the baseline can pass while the tree is broken.
find app/src fw/src -type f -exec touch {} +

echo "=== baseline — everything that must compile, compiled together ==="
echo "  E2/E2b (extension trait) · D2/D2b/D3 · V2 · P1/P2/P3 · G1 (the hole)."
echo "  A red baseline makes every row below unreadable."
if "${CARGO[@]}" check -p app --all-targets 2>&1 | grep -q 'Finished'; then
    printf '  %-6s %-6s %-6s  %s\n' "BASE" "pass" "pass" "as specified"
    pass=$((pass + 1))
else
    echo "FATAL: the baseline does not compile. Nothing below is interpretable." >&2
    "${CARGO[@]}" check -p app --all-targets 2>&1 | sed 's/^/  | /' | head -20
    exit 1
fi
echo

echo "=== (a) where can the getter live? ==="
# E1 is the correction: the shape the first version of this spike measured is an
# inherent impl on a foreign type once `Repo` and `Domain` are in different
# crates, which is the real layering. RK-004.
probe E1   fail 'E0116'   check -p app --features e1-inherent-impl-foreign-type
# E3's control is E2b, in the baseline: the same method against a set that
# contains the element.
probe E3   fail 'E0277'   check -p app --features e3-undeclared-read
echo

echo "=== (b) is the extension trait forgeable? ==="
# F1 is expected to PASS, and that is the finding. It is also F2's control:
# the only difference between them is the trait's type parameter.
probe F1   pass 'Finished' check -p app --features f1-forge-parameterised-trait
probe F2   fail 'E0119'    check -p app --features f2-forge-associated-trait
probe G2   fail 'E0117'    check -p app --features g2-repoint-readset
echo

echo "=== (c) the Domain-side shape ==="
# D1's control is D2, in the baseline: the same read succeeds once `R` is named.
probe D1   fail 'E0283'   check -p app --features d1-domain-getter-infer
# D2c's control is D2/D2b: the turbofish escapes inference, not the check.
probe D2c  fail 'E0277'   check -p app --features d2c-turbofish-undeclared
echo

echo "=== (d) what breaks — the view conversion ==="
# V1's control is V2, in the baseline: the same conversion through a plain getter.
probe V1   fail 'E0283'   check -p app --features v1-view-from-checked-getter
echo

echo "=== (e) can a Projection's Debug narrow to the declared set? ==="
# P3's control pair. P3b is the missing half of "the projection also enforces".
probe P3b  fail 'E0277'   check -p app --features p3b-projection-undeclared

# P4 is the one probe whose result is OUTPUT, not an exit code. Asserting only
# that it runs would prove nothing — the claim is about which fields print.
P4="$("${CARGO[@]}" run -q -p app --bin p4-projection-debug 2>&1)"
P4_WANT_1='1 field : Projection { email: "e@x" }'
P4_WANT_2='2 fields: Projection { email: "e@x", name: "nm" }'
if grep -qF -- "$P4_WANT_1" <<<"$P4" \
    && grep -qF -- "$P4_WANT_2" <<<"$P4" \
    && ! grep -qF -- 'SHOULD-NOT-APPEAR' <<<"$P4"; then
    printf '  %-6s %-6s %-6s  %s\n' "P4" "pass" "pass" "as specified"
    pass=$((pass + 1))
else
    printf '  %-6s %-6s %-6s  %s\n' "P4" "pass" "fail" "UNEXPECTED — output mismatch"
    sed 's/^/       | /' <<<"$P4"
    fail=$((fail + 1))
fi
echo

echo "=== summary ==="
if [[ $fail -gt 0 ]]; then
    echo "  result: $pass as specified, $fail unexpected" >&2
    exit 1
fi

# The deleted-row guard. Without it, removing a `probe` line leaves the suite
# green with one less thing measured (#43's lesson, re-planted in #48).
EXPECTED_ROWS=11
if [[ $pass -ne $EXPECTED_ROWS ]]; then
    echo "FATAL: $pass rows ran, expected $EXPECTED_ROWS — a probe line was removed." >&2
    exit 1
fi

echo "  result: $pass as specified, 0 unexpected"
echo
echo "See README.md for what these mean for docs/specs/read-contract.md and"
echo "docs/adr/0004-reads-enforcement-level.md."
