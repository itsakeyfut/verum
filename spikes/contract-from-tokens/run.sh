#!/usr/bin/env bash
#
# T-M1-07 / #37 — can token-scanning `handle` recover the Contract?
#
# The verdict here is **output**, not exit status. Every positive probe asserts
# the emitted JSON in full, because "the macro produced something" is not "the
# macro produced the contract" — and that gap is the failure this project keeps
# repeating. A partial assertion would have passed while `User::email` was being
# reported both inside the `when` scope and at the top level, which is exactly
# what the first run of this harness caught.
#
# NOT A CI GUARD, and deliberately not wired into any workflow — the same
# treatment as the four sibling spikes. It is out of the root workspace so that
# two probes designed not to compile never reach `cargo check --workspace`.
#
# WHAT WOULD OTHERWISE CORRUPT THE RESULT — `docs/rules/test.md` §9
#   §9-1  Each rejection asserts its own error code, and each acceptance asserts
#         its exact JSON.
#   §9-3  `Finished`, never `Checking <crate>` — the latter is cache-dependent.
#   §9-4  `[lints.rust] unexpected_cfgs = "deny"` per package, so a misspelled
#         feature name in the SOURCE is an error rather than a silent no-op.
#         Needles are error codes or exact JSON, never prose cargo could also
#         emit — the collision measured in #15.
#   §9-5  The baseline runs after a recursive `touch`. Measured: removing it
#         leaves the suite green here, because each probe uses a distinct
#         feature set and cargo rebuilds anyway. Kept as insurance, but it is
#         not load-bearing in this spike — unlike domain-opacity-sqlx, where DB
#         state made it so.
#   §9-7  That touch recurses; a new subdirectory cannot disarm it.
#   §9-12 The toolchain is asserted, and installation is checked FIRST so the
#         assertion is not self-referential (#17's finding: rustup silently
#         downloads a missing toolchain).
#   §9-13 `src/bin/observed.rs` names every `__VERUM_OBSERVED_*` const, so
#         deleting a probe is `E0425` at the bin and the baseline goes FATAL.
#         An earlier version also carried a `const _` block for this; it was
#         removed after planting showed deleting one left the suite green — the
#         bin was already the anchor, and a check that cannot fail is not one.
#   §9-14 NOOP is the standing control for every positive probe: a macro that
#         emitted a constant would pass all six. SneakyControl is P4's control.
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
# NOT `--no-deps`: that skips dependency resolution, so an unresolvable tree
# returns 0 and the preflight cannot fire for the reason it names (measured).
if ! "${CARGO[@]}" metadata --format-version 1 >/dev/null 2>&1; then
    echo "FATAL: the dependency tree cannot be resolved. The first probe's" >&2
    echo "       failure would otherwise be reported as a spike result." >&2
    exit 1
fi
printf '  crates       mac (the attribute macro) <- app (handler-rules.md, verbatim)\n'
echo

# A rejection probe: expected to fail, carrying a specific error code.
reject() {
    local id="$1" needle="$2"
    shift 2
    local out rc=0
    out="$("${CARGO[@]}" "$@" 2>&1)" || rc=$?
    if [[ $rc -ne 0 ]] && grep -qF -- "$needle" <<<"$out"; then
        printf '  %-6s %-24s %s\n' "$id" "$needle" "as specified"
        pass=$((pass + 1))
    else
        printf '  %-6s %-24s %s\n' "$id" "$needle" "UNEXPECTED (rc=$rc)"
        fail=$((fail + 1))
        grep -E '^error' <<<"$out" | head -3 | sed 's/^/       | /'
    fi
}

# An output probe: the emitted JSON must match in full.
expect() {
    local id="$1" endpoint="$2" want="$3"
    local got
    got="$(grep -P "^${endpoint}\t" <<<"$OBSERVED" | cut -f2- || true)"
    # `-n "$got"` matters: a missing endpoint yields "", and a future empty
    # `want` would then match it. No probe should ever assert emptiness here.
    if [[ -n "$got" && "$got" == "$want" ]]; then
        printf '  %-6s %-14s %s\n' "$id" "$endpoint" "as specified"
        pass=$((pass + 1))
    else
        printf '  %-6s %-14s %s\n' "$id" "$endpoint" "UNEXPECTED"
        printf '       | want: %s\n       | got : %s\n' "$want" "$got"
        fail=$((fail + 1))
    fi
}

# §9-5 / §9-7.
find app/src mac/src -type f -exec touch {} +

echo "=== baseline — every probe crate compiles ==="
if "${CARGO[@]}" check -p app --all-targets 2>&1 | grep -q 'Finished'; then
    printf '  %-6s %-24s %s\n' "BASE" "Finished" "as specified"
    pass=$((pass + 1))
else
    echo "FATAL: the baseline does not compile. Nothing below is interpretable." >&2
    "${CARGO[@]}" check -p app --all-targets 2>&1 | sed 's/^/  | /' | head -20
    exit 1
fi
echo

echo "=== what the macro cannot be given ==="
# R1's control is every other endpoint: the same attribute on an impl block works.
reject R1 "goes on an \`impl\` block" check -p app --features r1-observe-on-a-struct
# R2 is why P7 is structural rather than incidental.
reject R2 "E0407"                    check -p app --features r2-helper-in-the-observed-block
# D2 — #42 defect 1's other half. `if false` does NOT relieve the declaration
# obligation, so a declared-but-dead effect satisfies the upper bound AND appears
# in the lower one: `declared \ observed` is empty and the CI gate reports nothing.
# This crate's `Ctx` carries no effect set, so the minimal `Has` shape stands in.
reject D2 "E0277"                    check -p app --features d2-dead-code-still-declared
echo

OBSERVED="$("${CARGO[@]}" run -q -p app --bin observed 2>/dev/null)"

echo "=== what the macro recovers, and what it invents — exact output ==="
echo "  mutates / when-scope / after_commit / P4 blindness"
expect P1-4   UpdateUser      \
'{"endpoint":"UpdateUser","fields":["User::name@top","User::email@when:EmailChanged"],"reads":["User@top"],"creates":["AuditLog@top"],"emits":["EmailVerificationRequested@when:EmailChanged","UserUpdated@top"],"calls":["Email@after_commit"],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  control: the same emit written inline IS seen"
expect P4c    SneakyControl   \
'{"endpoint":"SneakyControl","fields":[],"reads":[],"creates":[],"emits":["HiddenEvent@top"],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  the escape hatch is visible though it is not an effect"
expect P5     EscapeHatch     \
'{"endpoint":"EscapeHatch","fields":[],"reads":[],"creates":[],"emits":[],"calls":[],"escapes":["User::from_repr@top"],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  aliasing: reads proves the scan ran; the setter is missed"
expect P6     Aliased         \
'{"endpoint":"Aliased","fields":[],"reads":["User@top"],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  a helper in a SIBLING impl block"
expect P7     ViaHelper       \
'{"endpoint":"ViaHelper","fields":[],"reads":["User@top"],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  the same helper as a nested fn — VISIBLE, so P7 is placement, not law"
expect X1     NestedFnHelper  \
'{"endpoint":"NestedFnHelper","fields":["User::name@top"],"reads":["User@top"],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  macro_rules! expansion — invisible by construction"
expect M1     MacroExpanded   \
'{"endpoint":"MacroExpanded","fields":[],"reads":["User@top"],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  UFCS, written directly in handle, no indirection"
expect U1     Ufcs            \
'{"endpoint":"Ufcs","fields":[],"reads":["User@top"],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  the parameter renamed to cx — everything vanishes"
expect V1     RenamedCtx      \
'{"endpoint":"RenamedCtx","fields":[],"reads":[],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  a never-compiled statement APPEARS: not a subset"
expect V2     CfgGated        \
'{"endpoint":"CfgGated","fields":[],"reads":[],"creates":[],"emits":["ThisTypeDoesNotExist@top"],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  nested when carries BOTH conditions"
expect W1     NestedWhen      \
'{"endpoint":"NestedWhen","fields":["User::email@when:EmailChanged+when:AlsoVerified"],"reads":["User@top"],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  #42 defect 1 — dead code: BOTH bounds count it"
# D1, the lower-bound half: `if false` is a token, so the effect appears exactly as
# an unconditional one does — note `@top`, with no condition tag. Contrast V2 above,
# which is code never *compiled*; this is code compiled and never *run*.
expect D1     DeadCode        \
'{"endpoint":"DeadCode","fields":[],"reads":[],"creates":[],"emits":["UserUpdated@top"],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'

echo "  the standing control: a constant-emitting macro fails here"
expect NOOP   Noop            \
'{"endpoint":"Noop","fields":[],"reads":[],"creates":[],"emits":[],"calls":[],"escapes":[],"scope":"ctx_spelled_same_item","deferred":"unknown"}'
echo

echo "=== summary ==="
if [[ $fail -gt 0 ]]; then
    echo "  result: $pass as specified, $fail unexpected" >&2
    exit 1
fi

EXPECTED_ROWS=17
if [[ $pass -ne $EXPECTED_ROWS ]]; then
    echo "FATAL: $pass rows ran, expected $EXPECTED_ROWS — a probe line was removed." >&2
    exit 1
fi

echo "  result: $pass as specified, 0 unexpected"
echo
echo "See README.md for what these mean for docs/specs/effect-inference.md and"
echo "docs/specs/rust-type-model.md."
