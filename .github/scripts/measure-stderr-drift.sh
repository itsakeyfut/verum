#!/usr/bin/env bash
#
# Measure how much Verum's `.stderr` expectations drift across rustc versions.
#
# WHY THIS EXISTS
# ---------------
# UI tests are Verum's primary test layer (docs/rules/test.md §1). Their errors
# embed cons lists and `There<There<..>>` index chains — compiler-formatted text
# that upstream is free to reshape. If that churns on every toolchain bump, the
# reflex becomes `TRYBUILD=overwrite`, and the tests stop protecting the error
# quality they exist to protect. #7 and #9 each produced a real instance of that
# reflex causing damage.
#
# The CI split in .github/workflows/ci.yml (MSRV authoritative, stable advisory)
# assumes an answer to "how much does it drift". This script produces it.
#
# NOT A CI GUARD. Deliberately not wired into any workflow: it is an
# investigation tool. A bug in a guard creates a false sense of safety; a bug
# here only produces bad data that a human reads. Re-run it when raising the
# MSRV, which is a breaking change under the version policy
# (.claude/commands/bump.md).
#
# USAGE
#   bash .github/scripts/measure-stderr-drift.sh            # as shipped
#   bash .github/scripts/measure-stderr-drift.sh --no-dnr   # with every
#                                                           # do_not_recommend stripped
#
# METHOD, and the three things that would otherwise corrupt the result
# --------------------------------------------------------------------
#  1. `--locked`. Cargo.lock pins trybuild 1.0.119; without it, `resolver = "3"`
#     on a newer toolchain can resolve a newer trybuild, and trybuild's own
#     changes would be reported as rustc drift.
#  2. The BASELINE row must come out at zero. It regenerates the very files the
#     committed baseline was generated from, so any difference means the harness
#     is broken, not that rustc drifted. The script refuses to report the rest of
#     the table until that holds — RK-016's lesson, learned by getting it wrong.
#  3. Below-MSRV toolchains are excluded. edition 2024 needs 1.85; 1.83 failing
#     is a hard floor, not drift.
#
# The real repository is never mutated: every toolchain runs in its own scratch
# copy made with `git archive`, so an uncommitted working tree cannot leak in and
# silently become the baseline. That also means **this script measures HEAD** —
# commit before running it, or you will be measuring the previous state.
set -euo pipefail

BASELINE="1.85.0"
TOOLCHAINS=(1.85.0 1.89.0 1.90.0 1.91.1 1.92.0 1.93.0 1.95.0 1.96.1 1.97.1 nightly)

STRIP_DNR=0
[[ "${1:-}" == "--no-dnr" ]] && STRIP_DNR=1

REPO="$(git rev-parse --show-toplevel)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

FIXTURES="crates/verum/tests/ui/compile_fail"

git -C "$REPO" archive HEAD | (mkdir -p "$WORK/pristine" && tar xf - -C "$WORK/pristine")

if [[ $STRIP_DNR -eq 1 ]]; then
    # One arm of the experiment: does do_not_recommend reduce *drift*, as distinct
    # from reducing line count (already known that it does)?
    find "$WORK/pristine/crates/verum/src" -name '*.rs' -print0 |
        xargs -0 sed -i '/#\[diagnostic::do_not_recommend\]/d'
fi

# ---------------------------------------------------------------------------
# Regenerate every .stderr under one toolchain, into its own tree.
# ---------------------------------------------------------------------------
regenerate() {
    local tc="$1" dir="$2"
    cp -r "$WORK/pristine" "$dir"
    # A shared CARGO_TARGET_DIR across toolchains would thrash; a per-toolchain
    # one costs disk but keeps each measurement independent.
    # The toolchain must exist. Without this the run below simply fails, every
    # .stderr stays exactly as copied, `compare` reports 0, and the row becomes
    # indistinguishable from "no drift" — measured: a bogus toolchain printed
    # `0 0 ok`. Worse, the same mechanism defeats the baseline self-check below,
    # so the whole table could look authoritative while nothing had run.
    if ! cargo "+$tc" --version >/dev/null 2>&1; then
        echo "FATAL: toolchain $tc is not installed." >&2
        echo "A missing toolchain would otherwise be reported as zero drift." >&2
        exit 1
    fi
    (
        cd "$dir"
        CARGO_TARGET_DIR="$WORK/target-$tc" TRYBUILD=overwrite \
            cargo "+$tc" test -p verum --test ui --locked >/dev/null 2>&1 || true
    )
    # And the run must actually have produced the test binary.
    #
    # The `|| true` above is deliberate: trybuild exits non-zero whenever an
    # expectation moves, which is the thing being measured. So the exit status
    # cannot separate "ran and found drift" from "never ran at all". This can.
    if ! find "$WORK/target-$tc" -name "ui-*" -type f 2>/dev/null | grep -q .; then
        echo "FATAL: $tc produced no ui test binary — the run did not happen." >&2
        echo "Every .stderr would still be the copied original, i.e. zero drift." >&2
        exit 1
    fi
}

# Compare one tree's .stderr against the reference set. Prints "<files> <lines>".
compare() {
    local ref="$1" dir="$2" files=0 lines=0
    for f in "$ref/$FIXTURES"/*.stderr; do
        local name
        name="$(basename "$f")"
        local other="$dir/$FIXTURES/$name"
        if [[ ! -f "$other" ]]; then
            files=$((files + 1))
            continue
        fi
        if ! diff -q "$f" "$other" >/dev/null 2>&1; then
            files=$((files + 1))
            lines=$((lines + $(diff "$f" "$other" | grep -cE '^[<>]' || true)))
        fi
    done
    printf '%s %s\n' "$files" "$lines"
}

# ---------------------------------------------------------------------------
# The reference set.
#
# As shipped, it is the committed .stderr. With --no-dnr the committed files no
# longer describe the code, so the arm needs its own reference: regenerate on the
# baseline toolchain and measure drift against *that*. Otherwise the arm would
# report the do_not_recommend delta and the rustc delta added together.
# ---------------------------------------------------------------------------
if [[ $STRIP_DNR -eq 1 ]]; then
    echo "arm: do_not_recommend stripped — building a no-dnr reference on $BASELINE"
    regenerate "$BASELINE" "$WORK/reference"
    REF="$WORK/reference"
else
    echo "arm: as shipped — reference is the committed .stderr"
    REF="$WORK/pristine"
fi

TOTAL_FIXTURES=$(find "$REF/$FIXTURES" -name '*.stderr' | wc -l | tr -d ' ')
if [[ "$TOTAL_FIXTURES" -lt 25 ]]; then
    echo "FATAL: only $TOTAL_FIXTURES .stderr in the reference set." >&2
    echo "The glob has drifted, so this script would report 'no drift' by not looking." >&2
    exit 1
fi
echo "reference holds $TOTAL_FIXTURES .stderr files"
echo

# ---------------------------------------------------------------------------
# Baseline row first, and refuse to continue if it is not zero.
# ---------------------------------------------------------------------------
regenerate "$BASELINE" "$WORK/run-$BASELINE"
read -r bf bl <<<"$(compare "$REF" "$WORK/run-$BASELINE")"
if [[ "$bf" -ne 0 ]]; then
    echo "FATAL: the $BASELINE row shows $bf changed files ($bl lines)." >&2
    echo "That row regenerates the reference itself, so this is a harness fault," >&2
    echo "not rustc drift. Every other row would be untrustworthy. Stopping." >&2
    exit 1
fi
echo "baseline self-check: $BASELINE reproduces the reference exactly (0 files)"
echo

printf '%-10s %8s %8s   %s\n' "toolchain" "files" "lines" "pass fixtures"
printf '%-10s %8s %8s   %s\n' "---------" "-----" "-----" "-------------"

for tc in "${TOOLCHAINS[@]}"; do
    dir="$WORK/run-$tc"
    [[ -d "$dir" ]] || regenerate "$tc" "$dir"
    read -r f l <<<"$(compare "$REF" "$dir")"

    # A `pass` fixture has no .stderr, but it can still stop compiling on a newer
    # toolchain. That is the type model breaking, not drift, so it goes in its own
    # column rather than being folded into the counts.
    passes="ok"
    if (cd "$dir" && CARGO_TARGET_DIR="$WORK/target-$tc" \
        cargo "+$tc" test -p verum --test ui --locked 2>&1 |
        sed 's/\x1b\[[0-9;]*m//g' | grep -qE '^test tests/ui/pass/.*(error|mismatch)'); then
        passes="BROKEN"
    fi

    printf '%-10s %8s %8s   %s\n' "$tc" "$f" "$l" "$passes"

    # Keep the first drifting tree so the diffs can be classified by hand.
    if [[ "$f" -ne 0 && ! -e "$WORK/keep" ]]; then
        cp -r "$dir/$FIXTURES" "${DRIFT_KEEP:-/tmp}/drift-sample-$tc" 2>/dev/null || true
        touch "$WORK/keep"
    fi
done

echo
echo "Interpretation: 'files' counts .stderr that differ at all; 'lines' is the"
echo "total of added+removed lines. Neither says *what* changed — classify the"
echo "diffs by hand (docs/rules/test.md §1.4 records the buckets), because the"
echo "strategy depends on whether Verum-authored text moved or only rustc's"
echo "scaffolding did. Set DRIFT_KEEP=<dir> to retain the first drifting tree."
