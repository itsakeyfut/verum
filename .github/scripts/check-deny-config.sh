#!/usr/bin/env bash
#
# Assert that deny.toml still carries the four settings that make
# `cargo deny check` non-vacuous.
#
# WHY THIS EXISTS
# ---------------
# cargo-deny reports `ok` for things it was not configured to look at, so a
# loosened deny.toml does not fail — it goes quiet. Two of the four settings below
# were measured to be load-bearing the hard way (T-M0-24):
#
#   * without `include-dev`, a `GPL-3.0-only` **dev**-dependency passes cleanly
#     (and `[graph] exclude-dev` does NOT control that, despite its name)
#   * without `unknown-git` / `unknown-registry` set explicitly, a git dependency
#     yields `warning[source-not-allowed]` and the check still reports
#     `sources ok` — so `cargo deny check sources` could not fail on the one thing
#     it is in the command line for. That shipped as far as code review.
#
# The realistic way these get loosened is not malice: it is unbreaking a red CI run
# by widening the config. Same argument as `unsafe_code = "forbid"` in Cargo.toml —
# relaxing a policy should always show up in a diff.
#
# WHAT THIS DOES NOT PROVE
# ------------------------
# That cargo-deny honours these keys. That half is established by measurement,
# recorded in deny.toml's own comments and in docs/dev/maintenance-tasks.md. This
# script only pins the config text, which here *is* the load-bearing artefact —
# unlike the seal annotations in sealed.rs, where text scanning checks a claim
# about the code and RK-016 warns against treating it as the defence itself.
#
# Widening the allow-list is deliberately NOT checked: that is the intended
# response to a new dependency, and it is visible in the diff.
#
# Usage: check-deny-config.sh [path/to/deny.toml]
# The argument exists so check-deny-config-test.sh can point it at mutated copies.
set -euo pipefail

readonly REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly CONFIG="${1:-$REPO/deny.toml}"

if [[ ! -f "$CONFIG" ]]; then
  echo "FAIL: $CONFIG does not exist." >&2
  echo "      cargo-deny would fall back to its default config, which allows" >&2
  echo "      nothing, so CI goes red — but the file is meant to be here." >&2
  exit 1
fi

# Comments are stripped first, so that a setting quoted inside prose (deny.toml's
# rationale mentions several) cannot satisfy a check.
readonly BODY="$(sed 's/#.*//' "$CONFIG")"

failed=0

# require <key> <value> <why it is load-bearing>
require() {
  local key="$1" value="$2" why="$3"
  if ! grep -Eq "^[[:space:]]*${key}[[:space:]]*=[[:space:]]*${value}[[:space:]]*$" <<<"$BODY"; then
    printf 'FAIL: deny.toml is missing `%s = %s`.\n' "$key" "$value" >&2
    printf '      %s\n' "$why" >&2
    failed=1
  fi
}

require include-dev true \
  'Without it a GPL-3.0-only dev-dependency passes cargo deny check licenses cleanly (measured).'
require unknown-git '"deny"' \
  'Without it a git dependency is only a warning and sources still reports ok (measured). Git deps sit outside the crates.io index, so RUSTSEC advisories and lockfile checksums do not apply to them.'
require unknown-registry '"deny"' \
  'Same as unknown-git, for a non-crates.io registry.'
require unused-allowed-license '"deny"' \
  'Without it the allow-list can keep permitting licences the tree no longer contains, and a rotting list cannot be told apart from a constraining one.'

if [[ $failed -ne 0 ]]; then
  echo >&2
  echo 'Loosening any of these makes cargo-deny quieter rather than red. If a change' >&2
  echo 'genuinely requires it, say why in deny.toml and update this guard in the same' >&2
  echo 'commit — the point is that it cannot happen silently.' >&2
  exit 1
fi

echo "ok: deny.toml carries all 4 settings that keep cargo deny from passing vacuously"
