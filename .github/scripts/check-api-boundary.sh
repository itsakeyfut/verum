#!/usr/bin/env bash
#
# Guards the Dependency Hiding Rule (docs/rules/api-surface.md §1) — the check
# that buys the freedom to drop Axum later. Two modes, run as two CI jobs:
#
#   imports     source level: where a forbidden crate may be named at all
#   public-api  the rendered public API of the `verum` crate
#
# Neither subsumes the other, and that is not redundancy. `cargo public-api`
# resolves types that reach the public surface through return positions and
# trait bounds, which no source grep can follow. But it renders
# `pub use axum::Router` as `pub use verum::Router` — provenance is erased — so
# re-exports are invisible to it, and only the source grep covers them.
#
# Run either mode locally; both are plain exit codes.
#
#   .github/scripts/check-api-boundary.sh imports
#   .github/scripts/check-api-boundary.sh public-api
#
# LIFECYCLE — this file does NOT go away with Axum.
#
# Phase 12 removes Axum and moves to hyper-util plus a hand-written router, and
# `hyper_util` / `tower` / `matchit` are already on the list below: the migration
# target is itself a forbidden crate. Only the axum entries and rule (a)'s target
# retire; the file stays.
#
# The reason is that §1 carries two independent rationales, and only one expires:
#
#   replaceability      "hiding buys the freedom to drop it later" — expires for
#                       axum once axum is gone
#   capability integrity a `State`-shaped entry point makes "an undeclared
#                       capability cannot be obtained" false. `tower::Service`
#                       takes any request and returns any response, so it breaks
#                       the contract exactly the way `axum::extract::State` does.
#                       This never expires, whatever the backend is.
#
# So at Phase 12: drop the axum roots, retarget rule (a) to whatever `runtime/`
# then wraps, and leave everything else. Deleting this script as "axum cleanup"
# would silently remove the capability-integrity guard — nothing would fail.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

# Pinned on purpose. rustdoc JSON is unstable and each cargo-public-api release
# understands only a window of nightlies — 0.46.x accepted an eight-day range.
# A floating nightly turns unrelated PRs red on the day the format changes.
# Bump these two together, never separately.
readonly RUSTDOC_NIGHTLY="nightly-2026-08-04"
readonly CARGO_PUBLIC_API_VERSION="0.52.0"

# Crate roots, not facades. `axum::response::IntoResponse` renders as
# `axum_core::response::into_response::IntoResponse`, so a list naming only
# `axum` would miss one of the two types api-surface.md §1 calls out as the
# most important to hide.
#
# `http` / `http_body` are absent deliberately: they are 1.x-stable foundations
# the hand-written runtime will use unchanged, so api-surface.md re-exports them
# rather than hiding them. `tokio` is absent because Verum is explicitly
# tokio-bound and does not pretend to be runtime-agnostic.
readonly FORBIDDEN_ROOTS=(
  axum axum_core axum_extra axum_macros
  tower tower_service tower_layer tower_http
  hyper hyper_util
  matchit
  sqlx sqlx_core sqlx_postgres sqlx_macros
)

# `runtime/` is the only module allowed to know about Axum (docs/rules/design.md §2).
readonly AXUM_ROOTS=(axum axum_core axum_extra axum_macros)

# `runtime/` legitimately names hyper, tower, tower-http and matchit — they are
# `verum`'s own dependencies and `design.md` §219 forbids them only *outside*
# `runtime/`. An earlier version of rule (c) barred them everywhere, which would
# have gone red on the first line of the real runtime; review caught it against
# `design.md` before `runtime/` existed to trip it.
#
# What `runtime/` has no reason to name is a database crate, and that is the hazard
# #34 found: a `quote!` in `verum-macros` hard-coding `sqlx::FromRow` makes every
# user require sqlx, invisibly — it appears in no manifest and never reaches
# `cargo public-api`, because generated tokens are not part of `verum`'s rendered
# API. So rule (c) is scoped to the database family alone.
#
# Derived, not hand-copied: an earlier version duplicated `FORBIDDEN_ROOTS` minus
# axum by hand, so adding a crate to one list silently left rule (c) not covering
# it. That is RK-016's rule 7 — derive the target names from the declaring side.
readonly DB_ROOTS=(sqlx sqlx_core sqlx_postgres sqlx_macros sqlx_sqlite)

readonly SCAN_ROOT="${VERUM_SCAN_ROOT:-crates}"
readonly RUNTIME_DIR="$SCAN_ROOT/verum/src/runtime"

alternation() {
  local IFS='|'
  echo "$*"
}

# Rust source files, excluding tests and build scripts.
rust_sources() {
  find "$SCAN_ROOT" -type f -name '*.rs' -path '*/src/*' | sort
}

# `//` line comments are stripped before matching so that prose mentioning axum
# is not a false positive. A `//` inside a string literal is also stripped; the
# only consequence is text that was never an import anyway.
strip_comments() {
  sed 's://.*$::' "$1"
}

fail() {
  echo "$@" >&2
}

check_imports() {
  local axum_re db_re reexport_re file body hits status=0 scanned=0
  # Rule (a) covers everything `runtime/` is allowed to name — axum AND the crates
  # `design.md` §219 forbids "outside `runtime/`" (tower, hyper_util, …). Before
  # this, (a) was axum-only, so `tower::` in `lib.rs` passed both rules; review
  # found the gap while checking that (c) had not over-reached the other way.
  mapfile -t RUNTIME_ONLY < <(
    comm -23 <(printf '%s\n' "${FORBIDDEN_ROOTS[@]}" | sort) \
             <(printf '%s\n' "${DB_ROOTS[@]}" | sort)
  )
  axum_re="(^|[^A-Za-z0-9_])($(alternation "${RUNTIME_ONLY[@]}"))::"
  # Everything forbidden that is NOT axum. `runtime/` has a documented reason to
  # name axum and none to name sqlx, so the two get different rules — see (c).
  db_re="(^|[^A-Za-z0-9_])($(alternation "${DB_ROOTS[@]}"))::"
  # `(::)?` matters: `pub use ::axum::Router;` is ordinary Rust, not obfuscation,
  # and inside runtime/ rule (a) permits axum — so without it that line passes
  # both rules.
  reexport_re="(^|[^A-Za-z0-9_])pub[[:space:]]+use[[:space:]]+(::)?($(alternation "${FORBIDDEN_ROOTS[@]}"))::"

  while IFS= read -r file; do
    scanned=$((scanned + 1))
    body=$(strip_comments "$file")

    # (a) The runtime-only families may be named only inside runtime/.
    case "$file" in
      "$RUNTIME_DIR"/*) ;;
      *)
        hits=$(printf '%s\n' "$body" | grep -nE "$axum_re" || true)
        if [ -n "$hits" ]; then
          fail "error: a runtime-only crate named outside ${RUNTIME_DIR}/ — $file"
          printf '%s\n' "$hits" | sed "s|^|    $file:|" >&2
          status=1
        fi
        ;;
    esac

    # (c) The rest of the forbidden list may not be named ANYWHERE in `crates/`,
    # runtime/ included. This is what catches `verum-macros` hard-coding
    # `sqlx::FromRow` into a `quote!`: the pass-through form decided in #34
    # forwards a path the *user* wrote, so a `sqlx` literal in Verum's own source
    # means Verum is requiring the dependency rather than carrying it. That never
    # reaches `cargo public-api`, because generated tokens are not part of
    # `verum`'s rendered API — measured in #34, and the reason this rule is here
    # rather than in check_public_api.
    hits=$(printf '%s\n' "$body" | grep -nE "$db_re" || true)
    if [ -n "$hits" ]; then
      fail "error: a database crate is named in Verum's own source — $file"
      printf '%s\n' "$hits" | sed "s|^|    $file:|" >&2
      status=1
    fi

    # (b) A forbidden crate may never be re-exported — including from runtime/.
    # Without this, runtime/ re-exports and lib.rs re-exports that, and the leak
    # passes both checks because cargo public-api has already lost provenance.
    hits=$(printf '%s\n' "$body" | grep -nE "$reexport_re" || true)
    if [ -n "$hits" ]; then
      fail "error: forbidden crate re-exported — $file"
      printf '%s\n' "$hits" | sed "s|^|    $file:|" >&2
      status=1
    fi
  done < <(rust_sources)

  # A failure inside the process substitution above does not propagate through
  # `set -e`. Without this, a renamed layout or a wrong working directory makes
  # the guard scan nothing and report success — "not checked" silently becomes
  # "nothing found", which is the one failure this guard must never have.
  if [ "$scanned" -eq 0 ]; then
    fail "error: scanned no Rust sources under ${SCAN_ROOT}/ — the guard did not run"
    return 1
  fi

  if [ "$status" -ne 0 ]; then
    fail ""
    fail "The Dependency Hiding Rule keeps Axum replaceable: hidden, the swap"
    fail "changes nothing for users; exposed once, it can never be removed."
    fail "See docs/rules/api-surface.md §1."
    return 1
  fi

  echo "ok: no forbidden crate named outside ${RUNTIME_DIR}/, none re-exported"
}

check_public_api() {
  if ! command -v cargo-public-api >/dev/null 2>&1; then
    fail "error: cargo-public-api is not installed"
    fail "  cargo install cargo-public-api --locked --version ${CARGO_PUBLIC_API_VERSION}"
    return 1
  fi

  # --omit drops blanket impls, auto-trait impls and auto-derived impls. They are
  # not noise-trimming here, they are correctness: a dependency's blanket impl
  # renders as `impl<A,B,T> hyper_util::...HttpServerConnExec for verum::Thing`
  # on *every* public type, which would fail this check for types that leak
  # nothing at all.
  local api
  api=$(RUSTUP_TOOLCHAIN="$RUSTDOC_NIGHTLY" cargo public-api \
    --omit blanket-impls,auto-trait-impls,auto-derived-impls \
    -p verum)

  echo "--- public API of verum ---"
  printf '%s\n' "$api"
  echo "---------------------------"

  # Same discipline as check_imports: an empty listing must not read as a clean
  # one. The crate root always renders, so its absence means nothing was checked.
  if ! printf '%s\n' "$api" | grep -q '^pub mod verum$'; then
    fail "error: cargo public-api produced no crate root — the guard did not run"
    return 1
  fi

  local forbidden_re hits
  forbidden_re="(^|[^A-Za-z0-9_])($(alternation "${FORBIDDEN_ROOTS[@]}"))::"
  hits=$(printf '%s\n' "$api" | grep -nE "$forbidden_re" || true)

  if [ -n "$hits" ]; then
    fail "error: foreign type in the public API of \`verum\`"
    printf '%s\n' "$hits" | sed 's|^|    |' >&2
    fail ""
    fail "http:: and http_body:: are allowed; the crates above are not."
    fail "See docs/rules/api-surface.md §1."
    return 1
  fi

  echo "ok: no forbidden crate in the public API"
}

case "${1:-}" in
  imports) check_imports ;;
  public-api) check_public_api ;;
  *)
    fail "usage: $0 {imports|public-api}"
    exit 2
    ;;
esac
