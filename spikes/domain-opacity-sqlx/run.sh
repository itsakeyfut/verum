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
# One judgement, used by both entry points. They were 22 of 24 lines identical,
# which is RK-016 rule (b)'s shape — a hand-copied guard whose original gets fixed
# and whose copy does not. `probe_absent` now differs only in its arguments.
_judge() {
    local id="$1" expect="$2" needle="$3" forbidden="$4" got="$5" out="$6"

    # A guard whose needle is the empty string matches everything, so the row would
    # judge nothing but the exit code. Measured in #44's review with a fake cargo.
    if [[ -z "$needle" ]]; then
        printf 'FATAL: probe %s has an empty needle — it would judge only the exit code.\n' "$id" >&2
        exit 1
    fi

    local reason="ok"
    if ! grep -qF -- "$needle" <<<"$out"; then
        reason="MISSING(\"$needle\")"
    elif [[ -n "$forbidden" ]] && grep -qF -- "$forbidden" <<<"$out"; then
        reason="PRESENT(\"$forbidden\") — the check fired after all"
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
probe() {
    local id="$1" expect="$2" needle="$3"
    shift 3
    local out rc=0
    out="$(cargo "$@" 2>&1)" || rc=$?
    local got="pass"
    [[ $rc -ne 0 ]] && got="fail"
    _judge "$id" "$expect" "$needle" "" "$got" "$out"
}

# probe_absent <id> <expect> <required substring> <FORBIDDEN substring> <cargo args...>
#
# For the rows whose finding is "it was rejected, but NOT by the check under
# test". Presence alone cannot express that: P42 expects `E0119` *and* expects
# verum's layer-1 wording to be missing, and a version that only needled on
# `E0119` would stay green if both appeared — i.e. if the name-based check started
# reaching the position it cannot see. The row would then report "the check is
# blind" while measuring the opposite.
probe_absent() {
    local id="$1" expect="$2" needle="$3" forbidden="$4"
    shift 4
    if [[ -z "$forbidden" ]]; then
        printf 'FATAL: probe_absent %s has an empty forbidden needle — it checks nothing.\n' "$id" >&2
        exit 1
    fi
    local out rc=0
    out="$(cargo "$@" 2>&1)" || rc=$?
    local got="pass"
    [[ $rc -ne 0 ]] && got="fail"
    _judge "$id" "$expect" "$needle" "$forbidden" "$got" "$out"
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
# The same route's second instance, and the one #13 asserted without compiling.
# `Deserialize` needs no database — a string literal is the whole attack.
probe P41 pass 'test result: ok. 1 passed' test -p separate-repo --features p41-forge-via-json --test forge
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

# --- WHICH MACRO FORM CAN BUILD ADR-0010's SHAPE? (#34) ---------------------
# P16 shows a derive cannot emit a SIBLING named after its input. ADR-0010's shape
# puts the struct inside a module, so the question had to be asked again. P38: the
# **re-export** collides, against the user's own item, which a derive cannot remove.
probe P38 fail 'E0255' check -p app --features p38-adr0010-from-derive
# P38's control, and the form #34 chose. The probe body references the expansion —
# review mutated the macro to emit NOTHING and an earlier version of this row stayed
# green, because a bare struct definition names none of the generated items.
probe P39 pass 'Finished' check -p app --features p39-adr0010-from-attribute
# ADR-0010's wall: the constructor is unreachable from outside the macro's module.
probe P39b fail 'E0624' check -p app --features p39b-attribute-forgery
# And the `Repr` is not nameable either — ADR-0010 marks it "module-private: paths
# 3/4 shut with it". Review found this row missing, and found that its absence had
# let the macro re-export the Repr AND expose `pub fn build(r: Repr)`, which together
# forged a domain from invented values, from a foreign crate included.
# Path-qualified on purpose: the first version wrote the unqualified name and
# got E0422, which only measures "omitted from the re-export". It stayed green
# while the macro emitted `pub(super) struct Repr` — reachable crate-wide.
probe P39d fail 'E0603' check -p app --features p39d-repr-not-nameable
# And from another module of the user's crate, which is exactly where
# `pub(super)` was reachable (P33's lesson, transposed onto the Repr).
probe P39e fail 'E0603' check -p app --features p39e-repr-not-nameable-elsewhere
# P39b/P39d's counter-evidence: the generated repository still works (ARK-002).
probe P39c pass 'Finished' check -p app --features p39c-legitimate-route
#
# P40 — THE REFUTATION OF ADR-0011's FIRST MECHANISM.
# A derive CAN own the confinement radius: emit only the `impl` block into the
# module, and a private inherent method's visibility is the module the `impl` sits
# in. No re-export, nothing collides. So "a derive cannot produce it" was false.
probe P40 pass 'Finished' check -p app --features p40-derive-can-confine
probe P40a fail 'E0624' check -p app --features p40a-derive-confine-forgery
# And the signature does not force the newtype either — owning a cached Repr works.
probe P19 pass 'Finished' check -p app --features p19-flat-cached-repr
echo

echo "=== #33 — can path 21 be closed? ==="
echo "  Four gates. The premise-falsifier is P2 above: app/src/repo.rs IS the"
echo "  repository, and P2 forges anyway — so generating it is not sufficient."
# The token gate. E0061 is an *arity* error: it rejects, but carries no wording
# Verum wrote, which is half of why requirement 2 (E0277) is unreachable.
probe P22 fail 'E0061' check -p app --features p22-token-missing
# The other half: the token reaches user code through a trait the user can
# implement — necessarily, since that is the trait the derive implements too.
probe P23 pass 'Finished' check -p app --features p23-token-stolen
# The bound gate, which is the only shape whose rejection could be E0277. It is
# not sealed, and cannot be: `impl fw::RepositoryProof for MyProof {}` is a
# foreign trait on a local type.
probe P24 pass 'Finished' check -p app --features p24-proof-forged
# ...and that impl is writable from any crate at all, not just the app's.
probe P25 pass 'Finished' check -p separate-repo --features p25-proof-forged-foreign
echo
echo "  ...and the one that survives: no modifier on from_repr, repository emitted"
echo "  into the domain's own module."
# THE MECHANISM. AccountRepr is pub(crate) so this fails on the *constructor*
# rather than on the type name — otherwise it would measure E0603 and prove
# nothing about from_repr.
probe P26 fail 'E0624' check -p app --features p26-confined-handler
# From a foreign crate. Both codes fire; E0603 is the one that arrives first, and
# it is the Repr's visibility rather than the confinement doing that work (same
# wall as P5). Stated so the mechanism is not credited with more than it does.
probe P27 fail 'E0624' check -p separate-repo --features p27-confined-foreign
# THE RESIDUE. A helper beside the user's own `struct Account` is inside the
# confinement and still forges. Without this row the table reads "closed".
probe P29 pass 'Finished' check -p app --features p29-same-module-forge
# Constraint 2: a module-private Repr takes ledger paths 3 and 4 with it — the
# name is unreachable, so there is no Debug to call. Contrast P20.
probe P30 fail 'E0603' check -p app --features p30-secret-repr-hidden
echo

echo "=== #33 round 2 — option D is dominated ==="
echo "  Review found two holes in the flat form and a mechanism without them."
# THE DECIDING PAIR. P29 (above) forges; P31 is the same shape under a derive-owned
# nested module and must be rejected. If P31 is red the residue is not unavoidable.
probe P31 fail 'E0624' check -p app --features p31-nested-user-helper
# The precondition nobody stated: at the crate root, "no modifier" IS pub(crate).
probe P33 pass 'Finished' check -p app --features p33-root-flat
# ...and the nested form does not care where the user put the domain.
probe P32 fail 'E0624' check -p app --features p32-root-nested
# The read half. Ledger path 21 names `as_repr()` too, and round 1 measured only
# the constructor while the specs claimed both.
probe P34 fail 'E0624' check -p app --features p34-nested-as-repr
# Constraint 2, measured with an actual Debug call. Round 1's P30 only NAMED the
# Repr and was insensitive to the derives it was cited as neutralising.
probe P35 fail 'E0624' check -p app --features p35-nested-repr-debug
# The constraint the decision depends on: trait-method visibility is the TRAIT's.
probe P36 pass 'Finished' check -p app --features p36-trait-defeats
# And the retraction: E0277 with Verum's wording is reachable after all. It is
# rejected for being unenforceable (P24/P25), not for being unavailable.
probe P37 fail 'cannot authorise constructing a Domain value' check -p app --features p37-proof-wording
echo

echo "=== #44 — the derives the USER attaches, and the insides of a field type ==="
echo "  ledger paths 26 and 28. Neither touches Repr, so neither is path 21, and"
echo "  both cross a crate boundary."
# P42 — the position the attribute cannot see. Rejected, but by a SHAPE mismatch:
# the derive's generated code names fields `#[domain]` replaced with a newtype.
# The absent-needle is what makes this row mean "the check never ran".
probe_absent P42 fail 'E0119' 'cannot be derived on a Domain' \
    check -p app --features p42-derive-above-attribute
# P47 — the same position, with the two spellings that defeated the name match:
# a raw ident and an aliased import. Both are `E0119`, because coherence does not
# read names. This is what stops the ledger describing the check as the defence.
probe P47 fail 'E0119' check -p app --features p47-spelling-independent
# P48/P49 — `Copy`. Emitting `Clone` to close path 26 removed the incidental
# `E0277` that had been stopping `Copy`, so closing two derives opened a third.
# P49 is the structural half (`E0204`, any position); P48 is the only route that
# gets past it, and it runs through the attribute's OWN argument list, where a
# check is position-independent by construction.
probe P48 fail 'cannot be forwarded to a Domain' check -p app --features p48-repr-copy-rejected
probe P49 fail 'E0204' check -p app --features p49-copy-blocked-structurally
# P50 — the residue: verum cannot name serde, so `Deserialize` has no collision
# available and the lint is all there is. Added because narrowing
# `FORBIDDEN_DERIVES` to `["Default"]` left the whole suite green.
probe P50 fail 'cannot be derived on a Domain' check -p app --features p50-deserialize-below
# P46 — the position it can see. #44's requirement 6, measured: the attribute form
# CAN enforce read-contract.md's ban. The trybuild fixture belongs with the real
# `#[domain]` (T-M2-04); this is what exists to be measured before it does.
probe P46 fail 'cannot be derived on a Domain' check -p app --features p46-derive-below-attribute
# P43/P44 — the pair that isolates the mechanism. Same shape, same derive, same
# position; the only difference is whether the attribute emits into a macro-owned
# child module. P43 compiles AND RUNS the forgery from a foreign crate; P44 is the
# same source rejected. So the closing mechanism is PLACEMENT — not the check (P42)
# and not the newtype shape (P43 keeps the fields and still forges).
probe P43 pass 'VERUM_P43_FORGED=attacker@example.com' \
    test -p separate-repo --features p43-forge-via-derive --test derives -- --nocapture
# `E0616` because `Clone` reads the field; with `Default` alone the same source is
# `E0451` (construction). Both measured in place — the code varies with the derive,
# as path 21's shape table already records for this class.
probe P44 fail 'E0616' check -p app --features p44-keep-shape-confined
# P45 — path 28. `type AuditTrail = RefCell<Vec<String>>` passes a name-based field
# whitelist, and the mutation goes through `&self`, so it is available where
# `Mutates = ()`. Built through its legitimate route: this is a correctly loaded
# Domain whose contents a foreign crate changes through a shared reference.
probe P45 pass 'VERUM_P45_AUDIT=written by a GET' \
    test -p separate-repo --features p45-mutate-through-shared-ref --test derives -- --nocapture
# P51/P53/P54/P52 — path 5's remedy, both horns and the way out, on the same input.
# The ledger's remedy column said the closing mechanism was `Freeze`; that is wrong
# in both directions, and these rows are what keep the replacement a measurement.
# P51: an allow-list of permitted field-type names rejects the user's own value
# object — too narrow, and unimplementable as specified.
probe P51 fail 'is not an allowed Domain field type' check -p app --features p51-allowlist-too-narrow
# P53: a deny-list of interior-mutable names ACCEPTS the alias. Too wide, in one
# token. P54 is its control — written out, the same check does reject it.
probe P53 pass 'Finished' check -p app --features p53-denylist-passes-alias
probe P54 fail 'carries interior mutability' check -p app --features p54-denylist-catches-direct
# P52: the same predicate emitted as a BOUND. `E0277`, because rustc resolves the
# alias for the macro — which is why "a derive sees only tokens" kills the name
# form and not the remedy. Its cost is that `Sync` also rejects `Rc`, and it does
# not reach `Mutex`/atomics; `Freeze` does not reach `Arc<Mutex<..>>` either.
probe P52 fail 'E0277' check -p app --features p52-bound-check-rejects-alias
echo

echo "=== does it actually work, not just type-check? ==="
# The count, not just `ok`: `cargo test -p app` runs three targets and two of them
# always report `test result: ok. 0 passed`, so the bare string matches even with
# roundtrip.rs deleted (measured). This is the only probe that answers "does it
# run", so it is the last one that should be able to pass vacuously.
probe P8 pass 'test result: ok. 2 passed' test -p app --features p2-from-repr --test roundtrip
# P28 — #33's `pass` case. A table of rejections proves nothing about the design
# still being usable, so the confined constructor is exercised against a real row
# through the repository the derive emits beside it.
probe P28 pass 'VERUM_P28_LOADED=alice@example.com' test -p app --test confined -- --nocapture
echo

# The deleted-row guard. This spike predates it (#43's lesson, re-planted in
# #48): without it, removing a `probe` line leaves the suite green with one less
# thing measured.
EXPECTED_ROWS=59
if [[ $((pass + fail)) -ne $EXPECTED_ROWS ]]; then
    printf 'FATAL: %d rows ran, expected %d — a probe line was removed.\n' "$((pass + fail))" "$EXPECTED_ROWS" >&2
    exit 1
fi

printf 'result: %d as specified, %d unexpected\n' "$pass" "$fail"
if [[ $fail -ne 0 ]]; then
    echo
    echo "An UNEXPECTED row means the verdict in README.md no longer describes this" >&2
    echo "toolchain / sqlx version. Re-read the diff before changing the specs." >&2
    exit 1
fi
echo
echo "See README.md for what these outcomes mean for docs/specs/persistence.md."
