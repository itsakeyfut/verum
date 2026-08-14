#!/usr/bin/env bash
#
# Regression test for check-api-boundary.sh `imports` mode.
#
# A guard needs both halves pinned. Fixtures that must fail prove it catches
# something; fixtures that must pass prove it is not simply rejecting everything
# — the same reason docs/rules/test.md §2 requires a `pass` test next to every
# `compile_fail` one.
#
# Two of these cases were live bugs found in review: `pub use ::axum::Router;`
# slipped through rule (b), and an empty scan reported success.
#
# `public-api` mode is not covered: its logic is a single grep over tool output,
# and a fixture would need a cargo workspace, an axum dependency and nightly.

set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly GUARD="$SCRIPT_DIR/check-api-boundary.sh"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

pass_count=0
fail_count=0

# write_fixture <relative-path> <content>
write_fixture() {
  local path="$TMP/crates/$1"
  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$2" > "$path"
}

reset_fixtures() {
  rm -rf "$TMP/crates"
  # A file that must never trip the guard, so every case below runs against a
  # non-empty tree and the "scanned nothing" check is not what makes it fail.
  write_fixture "verum/src/lib.rs" "//! verum"
}

# expect <expect-fail|expect-pass> <description>
expect() {
  local want="$1" desc="$2" got
  if VERUM_SCAN_ROOT="$TMP/crates" "$GUARD" imports >/dev/null 2>&1; then
    got="pass"
  else
    got="fail"
  fi
  if [ "$got" = "${want#expect-}" ]; then
    echo "  ok   $desc"
    pass_count=$((pass_count + 1))
  else
    echo "  FAIL $desc (expected ${want#expect-}, got $got)"
    fail_count=$((fail_count + 1))
  fi
}

echo "must be rejected:"

reset_fixtures
write_fixture "verum/src/lib.rs" "use axum::Router;"
expect expect-fail "axum imported outside runtime/"

reset_fixtures
write_fixture "verum/src/runtime/mod.rs" "pub use axum::Router;"
expect expect-fail "forbidden crate re-exported from runtime/"

reset_fixtures
write_fixture "verum/src/runtime/mod.rs" "pub use ::axum::Router;"
expect expect-fail "re-export with a leading :: (regression: rule (b) missed this)"

reset_fixtures
write_fixture "verum/src/persistence.rs" "pub use ::sqlx::PgPool;"
expect expect-fail "leading :: applies to every forbidden root, not just axum"

reset_fixtures
write_fixture "verum-macros/src/lib.rs" "use axum::Router;"
expect expect-fail "the boundary covers every crate, not only verum"

rm -rf "$TMP/crates"
expect expect-fail "empty scan (regression: reported success without checking)"

echo "must be accepted:"

reset_fixtures
write_fixture "verum/src/runtime/mod.rs" "use axum::response::IntoResponse;"
expect expect-pass "axum used inside runtime/ without re-exporting"

reset_fixtures
write_fixture "verum/src/lib.rs" "// runtime/ is the only module that may touch axum::Router"
expect expect-pass "prose mentioning axum in a comment"

reset_fixtures
write_fixture "verum/src/runtime/mod.rs" "pub use self::server::Server;"
expect expect-pass "local re-export inside runtime/"

reset_fixtures
write_fixture "verum/src/runtime/mod.rs" "pub(crate) use axum::Router;"
expect expect-pass "pub(crate) use is not public"

reset_fixtures
write_fixture "verum/src/lib.rs" "pub use http::{HeaderMap, StatusCode};"
expect expect-pass "http is a foundation crate api-surface.md wants re-exported"

echo
if [ "$fail_count" -ne 0 ]; then
  echo "check-api-boundary: $fail_count of $((pass_count + fail_count)) cases failed" >&2
  exit 1
fi
echo "check-api-boundary: all $pass_count cases behaved as specified"
