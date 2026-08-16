#!/usr/bin/env bash
#
# #43 — does every Rust code block in docs/ compile?
#
# Not a CI guard yet: it depends on a stub crate that stands in for a framework
# surface which does not exist, so a red run can mean "the docs are wrong" or
# "the stub is behind". Wiring it into CI is worth doing once M2 lands the real
# macros and the stub shrinks.
#
# WHAT WOULD OTHERWISE CORRUPT THE RESULT — docs/rules/test.md §9
#   §9-1  `compile_fail` blocks must actually fail, and are counted separately
#   §9-2  the expected counts are asserted, not printed
#   §9-4  blocks are compiled with `--test`. Without it a `#[test]` function is
#         stripped before type checking and its body is never compiled — this
#         harness shipped that hole and it was caught by suspecting the harness
#   §9-9  `extract.py` refuses an empty scan; a path bug found it once already
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

TOOLCHAIN="1.85.0"
RUSTC_V="$(rustc "+${TOOLCHAIN}" --version)"
[[ "$RUSTC_V" == rustc\ ${TOOLCHAIN}\ * ]] || { echo "FATAL: expected rustc ${TOOLCHAIN}, got $RUSTC_V" >&2; exit 1; }
echo "  rustc  $RUSTC_V"

cargo "+${TOOLCHAIN}" build -q
DEPS=target/debug/deps

echo
echo "=== inventory ==="
python3 extract.py

echo
echo "=== compile ==="
OUT="$(python3 check.py --deps "$DEPS")"
echo "$OUT"

# §9-2. `checked:BAD` is the remaining work; it must not grow. Lower it as blocks
# are fixed, and the diff records the progress.
EXPECTED_BAD=23
got_bad="$(awk '/checked:BAD/{print $2}' <<<"$OUT")"
got_bad="${got_bad:-0}"
if [[ "$got_bad" -gt "$EXPECTED_BAD" ]]; then
    echo "FATAL: $got_bad unmarked blocks fail, expected at most $EXPECTED_BAD" >&2
    exit 1
fi
if [[ "$got_bad" -lt "$EXPECTED_BAD" ]]; then
    echo "NOTE: down to $got_bad from $EXPECTED_BAD — lower EXPECTED_BAD in run.sh" >&2
fi
echo
echo "残り $got_bad ブロック。python3 check.py --deps $DEPS --verbose で一覧。"
