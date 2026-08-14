#!/usr/bin/env bash
#
# Regression test for check-deny-config.sh.
#
# A guard needs both halves pinned. Fixtures that must fail prove it catches
# something; fixtures that must pass prove it is not simply rejecting everything
# — the same reason docs/rules/test.md §2 requires a `pass` test next to every
# `compile_fail` one, and the same shape as check-api-boundary-test.sh.
#
# The comment-only case is the one worth knowing about: deny.toml explains at
# length *why* each setting is load-bearing, so those strings appear in prose. A
# guard that grepped the raw file would be satisfied by the rationale alone, with
# every real setting deleted.
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly GUARD="$SCRIPT_DIR/check-deny-config.sh"
readonly REAL="$SCRIPT_DIR/../../deny.toml"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

pass_count=0
fail_count=0

# expect <must_fail|must_pass> <name> <sed program applied to the real deny.toml>
expect() {
  local mode="$1" name="$2" program="$3"
  local file="$TMP/deny.toml"
  sed "$program" "$REAL" >"$file"

  # A sed program that matches nothing yields the real file back. On a must_fail
  # case that surfaces (the guard passes and the case is reported wrong), but on a
  # must_pass case it would silently report `ok` while proving nothing — the guard
  # would look tolerant of a form it had never seen. So assert the mutation landed.
  # RK-016: claim that the work happened, not that no error appeared.
  if [[ -n "$program" ]] && cmp -s "$file" "$REAL"; then
    printf '  FAIL %-52s the sed program matched nothing — fixture is unmutated\n' "$name"
    fail_count=$((fail_count + 1))
    return
  fi

  local rc=0
  "$GUARD" "$file" >/dev/null 2>&1 || rc=$?

  local got="pass"
  [[ $rc -ne 0 ]] && got="fail"
  local want="pass"
  [[ "$mode" == "must_fail" ]] && want="fail"

  if [[ "$got" == "$want" ]]; then
    printf '  ok   %-52s (%s)\n' "$name" "$got"
    pass_count=$((pass_count + 1))
  else
    printf '  FAIL %-52s want=%s got=%s\n' "$name" "$want" "$got"
    fail_count=$((fail_count + 1))
  fi
}

echo "check-deny-config.sh — fixtures that must be rejected:"
expect must_fail 'include-dev flipped to false'        's/^include-dev = true$/include-dev = false/'
expect must_fail 'include-dev line deleted'            '/^include-dev = true$/d'
expect must_fail 'unknown-git loosened to allow'       's/^unknown-git = "deny"$/unknown-git = "allow"/'
expect must_fail 'unknown-registry line deleted'       '/^unknown-registry = "deny"$/d'
expect must_fail 'unused-allowed-license downgraded'   's/^unused-allowed-license = "deny"$/unused-allowed-license = "warn"/'
# The whole [sources] table removed, which is how the defect this guard exists for
# actually looked: not a loosened value, an absent section.
expect must_fail 'entire [sources] table removed'      '/^\[sources\]$/,/^unknown-git = "deny"$/d'
# Every real setting gone, but the prose that mentions them left in place.
expect must_fail 'settings only present inside comments' 's/^\(include-dev\|unknown-git\|unknown-registry\|unused-allowed-license\) /# &/'

echo "check-deny-config.sh — fixtures that must be accepted:"
expect must_pass 'the real deny.toml, unmodified'      ''
expect must_pass 'no spaces around ='                  's/^include-dev = true$/include-dev=true/'
expect must_pass 'leading indentation'                 's/^include-dev = true$/  include-dev = true/'

# A missing file is its own case: there is nothing to sed.
if "$GUARD" "$TMP/definitely-not-here.toml" >/dev/null 2>&1; then
  printf '  FAIL %-52s want=fail got=pass\n' 'config file absent'
  fail_count=$((fail_count + 1))
else
  printf '  ok   %-52s (fail)\n' 'config file absent'
  pass_count=$((pass_count + 1))
fi

echo
if [[ $fail_count -ne 0 ]]; then
  echo "check-deny-config-test.sh: $fail_count of $((pass_count + fail_count)) cases behaved wrongly" >&2
  exit 1
fi
echo "check-deny-config-test.sh: all $pass_count cases behaved as specified"
