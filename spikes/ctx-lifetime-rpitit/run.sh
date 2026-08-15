#!/usr/bin/env bash
#
# T-M1-02 / #14 — is `Ctx<'req, E>` (`Send`, non-`'static`) compatible with an
# RPITIT `Handler`, a multi-thread hyper server, and `ctx.when`'s async closure?
#
# Every row below is a compile — or, for the five in the last section, a run —
# whose expected outcome is declared up front. The verdict is what this script
# prints, not what anyone reasoned: `docs/roadmap/M1-type-model-verification.md`
# requires M1's conclusions to come from a compiler.
#
# NOT A CI GUARD, and deliberately not wired into any workflow — the same
# treatment, and mostly the same reason, as `domain-opacity-sqlx/run.sh` and
# `.github/scripts/measure-stderr-drift.sh`. The difference is worth stating,
# because "spikes are independent" is the kind of summary that survives and the
# reason does not: sqlx *cannot* build on Verum's MSRV, so that spike had no
# choice. tokio and hyper build on 1.85.0 fine. This one is out of the workspace
# because a throwaway measurement should not put 21 further packages into the
# production dependency graph (30 resolved here, 7 already shared with the root),
# not because it could not go in.
#
# WHAT WOULD OTHERWISE CORRUPT THE RESULT — `docs/rules/test.md` §9
#   §9-1  A probe expected to fail can fail for the wrong reason. Each rejection
#         asserts its own expected message, not merely a non-zero exit.
#   §9-2  The runtime section asserts a count, because `test result: ok` matches
#         a file with every test deleted.
#   §9-3  `Finished`, never `Checking <crate>` — the latter only appears on a
#         cold build, so it makes the score depend on cache state.
#   §9-5  The baseline runs after a recursive `touch`, so a cached diagnostic
#         cannot replay as a pass.
#   §9-11 The baseline covers *both* crates that host probes.
#   §9-12 The toolchain is asserted, not printed. C1's message seeds an M3 UI
#         test and `docs/rules/test.md` §1.4 judges `.stderr` on 1.85.0; a text
#         captured on 1.97.1 would be the wrong specification.
#
# USAGE
#   bash run.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

TOOLCHAIN="1.85.0"
CARGO=(cargo "+${TOOLCHAIN}")

pass=0
fail=0

# ---------------------------------------------------------------------------
# Environment. §9-12: assert, do not merely print.
# ---------------------------------------------------------------------------
echo "=== environment ==="
RUSTC_V="$(rustc "+${TOOLCHAIN}" --version)"
printf '  rustc        %s\n' "$RUSTC_V"
if [[ "$RUSTC_V" != rustc\ ${TOOLCHAIN}\ * ]]; then
    echo "FATAL: expected rustc ${TOOLCHAIN}; a rustup override or a missing" >&2
    echo "       toolchain would silently measure a different compiler." >&2
    exit 1
fi

"${CARGO[@]}" metadata --format-version 1 >/dev/null 2>&1 || {
    echo "FATAL: the dependency tree cannot be resolved. The first probe's" >&2
    echo "       failure would otherwise be reported as a spike result." >&2
    exit 1
}
"${CARGO[@]}" metadata --format-version 1 2>/dev/null | python3 versions.py

# §9-5: a probe that never recompiled cannot be judged. §9-7: recursive, so
# adding `app/src/probes/` later does not silently disarm this.
find fw app -name '*.rs' -not -path '*/target/*' -exec touch {} +
echo

# ---------------------------------------------------------------------------
# probe <id> <expect pass|fail> <required substring> <cargo args...>
#
# The substring is the expected error message for a rejection, or `Finished` for
# an acceptance. It is what separates "rejected for the reason under test" from
# "rejected" — in #13 two predicted error codes were wrong and this is what
# caught them.
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Baseline. Everything below is read against "the design as specified compiles",
# so if that is false the rest of the table means nothing. §9-11: both crates.
# ---------------------------------------------------------------------------
echo "=== baseline — the design as the specs describe it, minus what D1 shows ==="
echo "  RPITIT Handler + erasure layer + Box<dyn> router + hyper Service,"
echo "  Ctx::new pub(crate), Repo without a lifetime, the two surviving when forms."
if ! "${CARGO[@]}" check -q -p fw -p app --all-targets >/dev/null 2>&1; then
    echo
    echo "FATAL: the baseline does not compile. Nothing below is interpretable." >&2
    "${CARGO[@]}" check -p fw -p app --all-targets 2>&1 | grep -E '^error' | head -10 >&2
    exit 1
fi
printf '  %-6s %-6s %-6s  %s\n' "BASE" "pass" "pass" "as specified"
pass=$((pass + 1))
echo

echo "=== (a) RPITIT Handler ==="
# A0 is the control for the whole suite: if `app` can build its own `Ctx`, every
# rejection below is walk-aroundable and the table proves nothing.
probe A0 fail 'E0624'                                    check -p app --features a0-forge-ctx
# A3 is the control for A1/A2: without it, a vacuous `Send` bound would pass.
probe A3 fail 'future cannot be sent between threads'    check -p app --features a3-non-send-body
echo

echo "=== (b) erasure layer, router, hyper ==="
# Two independent reasons a `Ctx` cannot appear in an erased signature, measured
# separately. Asserting only `E0038` would not tell them apart — and the first
# version of this suite did exactly that, so B2a's code was scored as evidence
# for a claim it does not support.
probe B2a fail 'because it requires `Self: Sized`'       check -p app --features b2a-erased-sized
probe B2b fail 'references the `Self` type in this parameter' \
                                                         check -p app --features b2b-erased-self-param
# B4a: the real constraint — `Service::call(&self, ..)` cannot return a future
# that borrows `self`. NOT "hyper forbids a non-'static future": B4b compiles in
# the baseline and shows it does not.
probe B4a fail 'lifetime may not live long enough'       check -p app --features b4a-borrow-from-self
echo

echo "=== (c) tokio::spawn — ledger paths 6 and 7 ==="
probe C1 fail 'E0521'                                    check -p app --features c1-spawn-ctx
probe C3 fail 'E0521'                                    check -p app --features c3-static-channel
echo

echo "=== when / AsyncFnOnce — RK-005 and ledger path 8 ==="
# D1 (the spec's elision) is in the baseline and runs in tests/live.rs.
# D1-bound is the footgun: writing that elision out by hand as a single
# higher-ranked lifetime changes what the bound demands.
probe D1b  fail 'not general enough'                     check -p app --features d1-bound-lifetimes
probe D1e  fail 'lifetime may not live long enough'      check -p app --features d1e-when-unboxed-fut
probe D2   fail 'E0499'                                  check -p app --features d2-when-capture
probe D3   fail 'E0308'                                  check -p app --features d3-when-leak-fixed-return
probe D4   fail 'lifetime may not live long enough'      check -p app --features d4-when-leak-unconstrained
# D5a/D5b isolate the variable: D5a attempts no leak at all. Both fail, so the
# named-lifetime form is not callable from a handler and D5b's rejection says
# nothing about leaking.
probe D5a  fail 'not general enough'                     check -p app --features d5a-when-named-call
probe D5b  fail 'not general enough'                     check -p app --features d5b-when-named-leak
echo

echo "=== #39 — capability handles and 'req (measured, not decided) ==="
probe E2 fail 'E0521'                                    check -p app --features e2-repo-lifetime-attack
probe E4a fail 'E0521'                                   check -p app --features e4-repo-phantom-attack
echo

echo "=== #40 — ctx.spawn (measured, not decided) ==="
probe F1 fail 'E0521'                                    check -p app --features f1-spec-spawn
echo

# ---------------------------------------------------------------------------
# The probes that have to run. Every endpoint in live.rs delegates to the
# function in app's lib rather than reimplementing it, so gutting a probe body
# turns a row red (docs/rules/test.md §9-13).
# ---------------------------------------------------------------------------
echo "=== does it run, not just type-check? ==="
probe B5+ pass 'test result: ok. 7 passed'               test -p app --test live
echo

# §9-2 applied to the harness itself, not only to the tests it runs: without
# this, deleting a `probe` line above leaves the suite green. Measured.
EXPECTED_ROWS=19
if [[ $pass -ne $EXPECTED_ROWS && $fail -eq 0 ]]; then
    echo "FATAL: $pass rows ran, expected $EXPECTED_ROWS — a probe line was removed." >&2
    exit 1
fi

printf 'result: %d as specified, %d unexpected\n' "$pass" "$fail"
if [[ $fail -ne 0 ]]; then
    echo
    echo "An UNEXPECTED row means the verdict in README.md no longer describes" >&2
    echo "this toolchain. Read the diff before changing any spec." >&2
    exit 1
fi
echo
echo "See README.md for what these outcomes mean for docs/specs/capability-system.md"
echo "and docs/specs/conditional-effects.md."
