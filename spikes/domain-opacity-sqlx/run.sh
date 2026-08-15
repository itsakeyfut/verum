#!/usr/bin/env bash
#
# T-M1-01 / #13 — does Domain opacity survive sqlx, and is the trust boundary
# where the specs say it is?
#
# Every row below is a compile (or, for P8, a run) whose expected outcome is
# declared up front. The verdict is what this script prints, not what anyone
# reasoned: `docs/roadmap/M1-type-model-verification.md` requires M1's conclusions
# to come from a compiler, and this project has been wrong in both directions
# about what rustc accepts.
#
# NOT A CI GUARD, and deliberately not wired into any workflow — same treatment,
# and the same reason, as .github/scripts/measure-stderr-drift.sh. sqlx 0.9.0
# declares rust-version 1.94.0 against Verum's MSRV of 1.85, so this cannot run on
# the toolchain that judges the workspace. A bug in a guard creates false safety; a
# bug here only produces bad data a human reads.
#
# THREE THINGS THAT WOULD OTHERWISE CORRUPT THE RESULT
#  1. A probe expected to fail can fail for the wrong reason — a typo, a missing
#     import — and be scored as "correctly rejected". So each rejection must carry
#     its expected error code, not merely a non-zero exit.
#  2. `query_as!` needs DATABASE_URL at compile time. Without the database, every
#     probe fails, and a table of failures reads like a finding. setup-db.py
#     asserts the table exists; this script asserts the baseline compiles before
#     reporting anything else.
#  3. The verdict is version-dependent. The versions actually measured are printed,
#     so a future divergence shows up as a changed number rather than silently.
#
# USAGE
#   bash run.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
ROOT="$PWD"
export DATABASE_URL="sqlite:$ROOT/spike.db"

pass=0
fail=0

# ---------------------------------------------------------------------------
# Environment, printed because the verdict is only true of these versions.
# ---------------------------------------------------------------------------
echo "=== environment ==="
printf '  rustc        %s\n' "$(rustc --version)"
python3 setup-db.py | sed 's/^/  /'
# Resolve sqlx's version now, and fail loudly if the tree cannot even be resolved
# — otherwise the first probe's failure would be reported as a spike result.
SQLX_VER="$(cargo metadata --format-version 1 2>/dev/null |
    python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="sqlx"))')"
if [[ -z "$SQLX_VER" ]]; then
    echo "FATAL: could not resolve sqlx's version — the dependency tree is broken." >&2
    exit 1
fi
printf '  sqlx         %s\n' "$SQLX_VER"
printf '  DATABASE_URL %s\n' "$DATABASE_URL"

# sqlx's macros verify SQL against the database at compile time, but neither the
# database file nor DATABASE_URL is a tracked input, so cargo will happily replay a
# cached diagnostic for a schema that no longer exists. Measured: breaking the
# schema without touching a source file left P1 green from cache and produced a
# table with 7 green rows and 5 red ones — worse than an all-red table, because the
# green rows read as "the boundary conclusions still hold". Forcing the leaf crates
# to recompile is what makes the baseline check below mean anything.
# `find`, not `app/src/*.rs`: a glob does not descend, so the moment anyone adds
# `app/src/domain/user.rs` the baseline check below would go quiet. That
# non-recursive-scan shape is the origin instance of the rule this line implements.
find fw mac app separate-repo -name '*.rs' -not -path '*/target/*' -exec touch {} +
echo

# ---------------------------------------------------------------------------
# probe <id> <expect pass|fail> <required substring> <cargo args...>
#
# The required substring is the error code for a rejection, or `Finished` for an
# acceptance. It is what separates "rejected for the reason under test" from
# "rejected" — P7 and P9 were both scored UNEXPECTED by it on the first run,
# because the codes I had predicted were wrong.
#
# `Finished` and not `Checking <crate>`: cargo only says `Checking` on a cold
# build, so the cold-cache version of this harness reported three spurious
# UNEXPECTED rows on its second run (measured). A harness whose answer depends on
# cache state is not one anybody can use to re-check a verdict.
# ---------------------------------------------------------------------------
probe() {
    local id="$1" expect="$2" needle="$3"
    shift 3

    local out rc=0
    out="$(cargo "$@" 2>&1)" || rc=$?

    local got="pass"
    [[ $rc -ne 0 ]] && got="fail"

    local reason="ok"
    if ! grep -qF -- "$needle" <<<"$out"; then
        reason="MISSING(\"$needle\")"
    fi

    if [[ "$got" == "$expect" && "$reason" == "ok" ]]; then
        printf '  %-4s %-6s %-6s  %s\n' "$id" "$expect" "$got" "as specified"
        pass=$((pass + 1))
    else
        printf '  %-4s %-6s %-6s  %s\n' "$id" "$expect" "$got" "UNEXPECTED — $reason"
        fail=$((fail + 1))
        sed 's/^/       | /' <<<"$out" | grep -E '^\s+\| (error|warning)' | head -6 || true
    fi
}

# ---------------------------------------------------------------------------
# Baseline first, and refuse to continue if it is not green. Everything below is
# interpreted relative to "the design as specified compiles", so if that is false
# the rest of the table means nothing.
# ---------------------------------------------------------------------------
echo "=== P1 — the design as the specs describe it ==="
echo "  query_as! + derive(FromRow) + query_as::<_,Repr> + pub(crate) from_repr/as_repr,"
echo "  repository in the same crate as the Domain."
if ! cargo check -q -p app -p separate-repo >/dev/null 2>&1; then
    echo
    echo "FATAL: the baseline does not compile. Nothing below is interpretable." >&2
    cargo check -p app -p separate-repo 2>&1 | grep -E '^error' | head -10 >&2
    exit 1
fi
echo "  P1   pass   pass    as specified"
pass=$((pass + 1))
echo

echo "=== is the trust boundary where the specs say it is? ==="
echo "  \"Repr を見られるのは Repository 実装だけ\" — persistence.md, mutation-contract.md"
# Reachable from ordinary handler code in the same crate: expected to COMPILE,
# which is the defect.
probe P2 pass 'Finished' check -p app --features p2-from-repr
probe P3 pass 'Finished' check -p app --features p3-as-repr
# The control. Opacity itself must still reject direct assignment, or a table of
# acceptances proves nothing.
probe P4 fail 'E0616' check -p app --features p4-direct-field
# Its companion: the same assignment against a `pub(crate)` inner field compiles.
# That is why the derive must emit a *private* inner field — grounding a
# recommendation that would otherwise be desk analysis.
probe P13 pass 'Finished' check -p app --features p13-pub-crate-inner
# P4's other missing companion: the same assignment from *inside* the module that
# defines the Domain. Field privacy is a module boundary, and the derive expands in
# the user's own module, so this is the permissive side of the line P4 measures.
probe P4b pass 'Finished' check -p app --features p4b-same-module-assign
# And the form the specs actually quote, which had no probe at all. The code depends
# on the surrounding shape, which is why three reviewers named three different ones:
#   newtype + a getter called `email`  -> E0615  (this is the real design's shape)
#   newtype, no such getter           -> E0609
#   flat struct, private named field, from outside the module -> E0616
# Only the last is the code four documents attributed to this line.
probe P18 fail 'E0615' check -p app --features p18-newtype-named-field
# The other reading: the repository in its own crate cannot see the Repr at all.
probe P5 fail 'E0603' check -p separate-repo --features p5-name-repr
echo

echo "=== constraints on the generated code ==="
# The spec's own `as_repr` signature forces User to be a newtype over its Repr.
probe P6 fail 'E0515' check -p app --features p6-flat-as-repr
# What query_as! needs is FIELD visibility at the call site, not struct visibility.
# E0451, not the E0616 of a field *access*: the macro expands into a struct literal.
probe P7 fail 'E0451' check -p app --features p7-private-repr-fields
echo

echo "=== constraints on the alternatives #18 will weigh ==="
# `E0446`: a public trait's associated type cannot be bound to a crate-private type.
# That is all this measures. It does NOT foreclose the projection route — P14 below
# opens it. An earlier version of this comment claimed it did.
probe P9 fail 'E0446' check -p app --features p9-trait-from-repr
# So the only listed alternative that can serve a repository in its own crate is
# "Repr public, fields private". These three measure exactly what it buys.
probe P10 pass 'Finished' check -p separate-repo --features p10-load
probe P11 fail 'E0451' check -p separate-repo --features p11-forge-pub-repr
probe P12 fail 'E0451' check -p separate-repo --features p12-macro-pub-repr
# …and it does not stop forging either. P11 rejects a struct *literal*; `FromRow`
# builds the struct inside `app`, from a row the caller supplies.
probe P15 pass 'test result: ok. 1 passed' test -p separate-repo --features p15-forge-via-select --test forge
# `E0446` fires because this spike wrote `pub(crate)`. Put the `Repr` in a private
# module as a `pub` type — the alternative the first README dismissed unprobed — and
# the trait route opens, projection and all. fw/src/lib.rs predicted this.
probe P14 pass 'Finished' check -p separate-repo --features p14-projection
# Deriving Debug/Clone on the Repr reopens ledger paths 4 and 3.
probe P20 pass 'Finished' check -p app --features p20-repr-debug-clone
echo

echo "=== can the macro the specs name actually emit the shape they specify? ==="
# The corrected spec put `pub struct User(UserRepr)` under a `// derive 生成`
# comment. A derive can only *add* items.
probe P16 fail 'E0428' check -p app --features p16-derive-newtype
# Control: the same macro emitting only the Repr. Without this, P16's failure could
# just mean the macro in this spike is broken.
probe P17 pass 'Finished' check -p app --features p17-derive-repr-only
# And the signature does not force the newtype either — owning a cached Repr works.
probe P19 pass 'Finished' check -p app --features p19-flat-cached-repr
echo

echo "=== does it actually work, not just type-check? ==="
# The count, not just `ok`: `cargo test -p app` runs three targets and two of them
# always report `test result: ok. 0 passed`, so the bare string matches even with
# roundtrip.rs deleted (measured). This is the only probe that answers "does it
# run", so it is the last one that should be able to pass vacuously.
probe P8 pass 'test result: ok. 2 passed' test -p app --features p2-from-repr --test roundtrip
echo

printf 'result: %d as specified, %d unexpected\n' "$pass" "$fail"
if [[ $fail -ne 0 ]]; then
    echo
    echo "An UNEXPECTED row means the verdict in README.md no longer describes this" >&2
    echo "toolchain / sqlx version. Re-read the diff before changing the specs." >&2
    exit 1
fi
echo
echo "See README.md for what these outcomes mean for docs/specs/persistence.md."
