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
#   §9-4  `[lints.rust] unexpected_cfgs = "deny"` is set per package, so a
#         misspelled feature name in a `#[cfg]` is an error rather than a probe
#         that silently compiles nothing.
#   §9-7  The baseline's cache invalidation recurses, so a new subdirectory
#         cannot disarm it.
#   §9-11 The baseline covers *both* crates that host probes.
#   §9-12 The toolchain is asserted, not printed. C1's message seeds an M3 UI
#         test and `docs/rules/test.md` §1.4 judges `.stderr` on 1.85.0; a text
#         captured on 1.97.1 would be the wrong specification.
#   §9-13 Pass probes are pinned at the call site where the type is nameable,
#         and every endpoint in `tests/live.rs` delegates to the function in
#         `app`'s lib rather than reimplementing it.
#   §9-14 Every rejection row has a standing control that removes the cause it
#         names — the pairs are tabulated in README.md.
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
    echo "FATAL: expected rustc ${TOOLCHAIN}; a TOOLCHAIN typo or a missing" >&2
    echo "       toolchain would silently measure a different compiler." >&2
    # NOT a rustup override: `rustc +1.85.0` already wins over one (measured).
    # What this catches is TOOLCHAIN="1.85", which resolves to 1.85.1, and a
    # toolchain that is not installed. `docs/rules/test.md` §9-12's own rationale
    # is a different mechanism — a copy outside the repository, where a
    # rust-toolchain file does not travel — and applies to bare `cargo`, not `+`.
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
# D5c/D5d isolate `+ Send` for the named-`'req` form. D5c drops the bound and
# nothing else: the leaking body type-checks, so #44's construction is real. D5d
# awaits it from a `+ Send` position and is rejected.
#
# DO NOT read D5d as "the leak has no caller" — an earlier version of the README
# did, and Tier-2 review refuted it by running the leak against the real server.
# `Handler::handle` is `fn .. -> impl Future + Send`, not `async fn`, so a
# synchronous body can drive a non-`Send` future beside the one it returns.
# `.await` is the only thing that propagates the obligation. See RK-017; the
# sync-body probe is NOT in this suite and should be added.
# Row 3 of the four-form table in docs/specs/conditional-effects.md. Rows 1 and
# 4 have had methods behind them since #14; rows 2 and 3 were published as
# "measured" while being desk analysis (#48). Row 2 is `d1r2_three_binders` in
# the default build — the baseline covers it, and a `pass` row asserting only
# `Finished` would assert nothing the exit code did not.
probe D1r3 fail 'not general enough'                     check -p app --features d1r3-two-shared

probe D5c  pass 'Finished'                               check -p app --features d5c-when-named-leak-nosend
probe D5d  fail 'not general enough'                     check -p app --features d5d-nosend-leak-from-handler
echo

echo "=== #39 — capability handles and 'req (measured, not decided) ==="
probe E2 fail 'E0521'                                    check -p app --features e2-repo-lifetime-attack
probe E4a fail 'E0521'                                   check -p app --features e4-repo-phantom-attack
# E5 / E5b — should the handle ALSO be `!Send`? E5 is the cost, E5b the porousness.
# The needle names `RepoNoSend` rather than the generic `Send` prose, which A3's
# row emits byte-identically — review re-pointed this row at A3's feature and it
# still read "as specified" with nothing under test compiled.
probe E5 fail 'RepoNoSend'                               check -p app --features e5-nosend-across-await
echo

echo "=== #40 — ctx.spawn (measured, not decided) ==="
probe F1 fail 'E0521'                                    check -p app --features f1-spec-spawn
# F4/F5/F6 — the third shape: the handler passes a payload and the framework builds
# the job's context inside the spawned task, borrowed from that task's own Runtime
# clone. F5 and F6 are the controls that make F4 mean something.
probe F5 fail 'E0521'                                    check -p app --features f5-scoped-job-respawn
probe F6 fail 'E0521'                                    check -p app --features f6-payload-smuggles-capability
echo

# ---------------------------------------------------------------------------
# The probes that have to run. Every endpoint in live.rs delegates to the
# function in app's lib rather than reimplementing it, so gutting a probe body
# turns a row red (docs/rules/test.md §9-13).
# ---------------------------------------------------------------------------
echo "=== does it run, not just type-check? ==="
probe B5+ pass 'test result: ok. 10 passed'               test -p app --test live
echo

# §9-2 applied to the harness itself, not only to the tests it runs: without
# this, deleting a `probe` line above leaves the suite green. Measured.
# The count is over pass+fail, and the check is UNCONDITIONAL. It used to be
# gated on `$fail -eq 0`, so any one red row switched off the guard whose whole
# job is to catch a deleted row — a fail-open found in #39's review (RK-016's
# eighth instance).
EXPECTED_ROWS=25
if [[ $((pass + fail)) -ne $EXPECTED_ROWS ]]; then
    printf 'FATAL: %d rows ran, expected %d — a probe line was removed.\n' \
        "$((pass + fail))" "$EXPECTED_ROWS" >&2
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
