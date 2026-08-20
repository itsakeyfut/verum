#!/usr/bin/env bash
# T-M1-xx / #41 — are the derive-facing seals forgeable once M2 exposes them?
#
# WHAT THIS ANSWERS
#   The ledger records that `#[doc(hidden)] pub mod __private` at M2 is "the moment
#   the seal weakens". This measures that it **stops working**, that the ledger's own
#   re-verification procedure is green on the tree where the forgery compiles, and
#   what the alternative shape does and does not buy.
#
# CONVENTIONS (docs/rules/test.md §9)
#   §9-1  Needles are rustc error codes or exact output — never prose cargo could
#         also emit for an unrelated reason.
#   §9-2  EXPECTED_ROWS is asserted unconditionally, so a deleted probe line cannot
#         hide behind another row being red.
#   §9-5  A baseline runs first, after a recursive `touch` (§9-7) so a probe cannot
#         pass on a stale artifact. If it does not compile, nothing below is meaningful.
#   §9-4  Every feature is declared, and `unexpected_cfgs = "deny"` is on, so a
#         typo'd `#[cfg]` is a hard error rather than a silently dead probe.
#   §9-13 **Every pass-expecting probe carries a same-feature `const _` pin.** The
#         first version of this file cited 9-1/2/3/4/14 and omitted 13 — and review
#         measured S2, S4 and S5 all staying green with their subjects deleted. The
#         rule it left out was the one it broke.
#   §9-14 Every rejection row has a control that removes the cause it names.
#
# USAGE
#   bash run.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

TOOLCHAIN="1.85.0"
CARGO=(cargo "+${TOOLCHAIN}")

pass=0
fail=0

echo "=== environment ==="
if ! rustup toolchain list | grep -q "^${TOOLCHAIN}"; then
    echo "FATAL: toolchain ${TOOLCHAIN} is not installed. rustup toolchain install ${TOOLCHAIN}" >&2
    exit 1
fi
printf '  rustc  %s\n' "$("${CARGO[@]}" --version)"

# §9-3. Also defeats caching, so a probe cannot pass on a stale artifact.
find . -name '*.rs' -exec touch {} +
echo "=== baseline ==="
if ! "${CARGO[@]}" check --workspace >/dev/null 2>&1; then
    echo "FATAL: the baseline does not compile. Nothing below is interpretable." >&2
    "${CARGO[@]}" check --workspace 2>&1 | grep -E '^error' | head -5 >&2
    exit 1
fi
echo "  ok — default features compile"

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

printf '\n  %-6s %-6s %-6s  %s\n' "probe" "expect" "got" "verdict"
printf '  %s\n' "------------------------------------------"

echo
echo "=== the shape that ships today: per-domain seals, exposed as M2 must ==="
# S1 is the ledger's recorded procedure, run verbatim. It is GREEN — and that is the
# finding, not a reassurance: S2 compiles on the same tree.
probe S1 fail 'cannot implement a sealed Verum trait' check -p downstream --features s1-trait-only
# S2 is the attacker who writes the seal too, because M2 made it public. Two
# undeclared domains pass the Architecture Contract, and are *used*, not merely
# declared.
probe S2 pass 'Finished' check -p downstream --features s2-seal-and-trait

echo
echo "=== #41's direction: Includes as a blanket impl ==="
# S3: the same attack, rejected — there is no seal to name, because with nothing
# emitted per domain the seal never leaves `fw`'s private module.
probe S3 fail 'does not declare the domain' check -p downstream --features s3-blanket-trait-only
# S4: #41's *stated* reason, on its own terms. Expected to COMPILE, refuting it —
# rustc judges a blanket impl and a competing specific impl disjoint exactly when the
# blanket's obligation is unsatisfiable, which is exactly the undeclared domain.
# CLAUDE.md records this as T-M0-08's lesson: coherence permits only the harmful side.
probe S4 pass 'Finished' check -p downstream --features s4-blanket-coherence
# S5 (§9-14): the control for S3. Without it, S3's rejection could just mean the
# blanket shape is unusable.
probe S5 pass 'Finished' check -p downstream --features s5-blanket-legitimate

echo
# §9-2: unconditional, and over pass+fail.
EXPECTED_ROWS=5
if [[ $((pass + fail)) -ne $EXPECTED_ROWS ]]; then
    printf 'FATAL: %d rows ran, expected %d — a probe line was removed.\n' \
        "$((pass + fail))" "$EXPECTED_ROWS" >&2
    exit 1
fi

printf 'result: %d as specified, %d unexpected\n' "$pass" "$fail"
if [[ $fail -ne 0 ]]; then
    echo "An UNEXPECTED row means the verdict in README.md no longer describes" >&2
    echo "this tree. Fix the README or the code — do not adjust the expectation." >&2
    exit 1
fi
