# Verum — testing standards

> **Verum's testing centres on UI tests (`trybuild`).** Error messages are a
> specification, and tests pin them.
> The canon of the design is
> [`../specs/diagnostics.md`](../specs/diagnostics.md) and
> [`../specs/evaluation.md`](../specs/evaluation.md).

## References

- [trybuild](https://docs.rs/trybuild)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Criterion](https://bheisler.github.io/criterion.rs/book/)

---

## Philosophy

Verum's value lies in **wrong code failing to compile**. So:

> **An ordinary test shows that correct code works.**
> **A UI test shows that wrong code is rejected, and that the reason is
> readable.**

The second is Verum's claim itself, and **breaking it is a spec violation.**

---

## 1. UI tests (`trybuild`) — the most important layer

### Introduced from the First PoC

Unless the type design and the error design are verified together, fixing the
wording afterwards is expensive.

```text
crates/verum/tests/
├── ui.rs                             ← the harness; it also holds the fixture-count floor
└── ui/
    ├── compile_fail/                     ← currently 30 (type-level forgery, seals, cons lists)
    │   ├── includes_manual_impl.rs        + .stderr   ← the seal (T-M0-06)
    │   ├── includes_undeclared_domain.rs  + .stderr   ← an unsatisfied bound at a use site
    │   ├── sealed_module_is_private.rs    + .stderr
    │   ├── sealed_second_path_is_private.rs + .stderr
    │   ├── has_cannot_be_forged*.rs / append_* / lookup_* / index_* …
    │   │                                              ← forgery cases added in T-M0-07..09
    │   ├── get_cannot_mutate.rs           + .stderr   ← everything below is **designed, not implemented**
    │   ├── undeclared_mutation.rs         + .stderr
    │   ├── direct_field_assignment.rs     + .stderr
    │   ├── pub_domain_field.rs            + .stderr
    │   ├── ctx_escapes_via_spawn.rs       + .stderr
    │   ├── when_scope_leak.rs             + .stderr
    │   ├── duplicate_declaration.rs       + .stderr
    │   └── forbidden_conflict.rs          + .stderr
    └── pass/
        └── includes_as_bound.rs
```

```rust,compile_fail
#[test]
fn contract_violations_should_not_compile() {
    // trybuild reports a glob that matches nothing as a pass. A directory rename
    // or a bad merge would silently reduce the primary test layer to zero, so
    // check the floor first (measured: an empty glob exits 0).
    //
    // The floor tracks the current count; it is not a low fixed value. A
    // comfortable floor like `3` means **the suite can be cut down to that value
    // and stay green** — measured, half the suite (including the two cases added
    // alongside the floor) could be deleted with CI still passing. Updating one
    // number per new fixture is the cheaper side of that trade.
    assert_fixtures_present("tests/ui/compile_fail", 30);
    assert_fixtures_present("tests/ui/pass", 4);

    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_fail/*.rs");
    t.pass("tests/ui/pass/*.rs");
}
```

### Violations that must be covered (First PoC)

| Case | What it proves | State |
|---|---|---|
| Calling `set_*` from a GET | The read-only guarantee on GET | designed |
| Mutating an undeclared field | The Mutation Contract | designed |
| Assigning `user.email = v` directly | Domain opacity | designed |
| A `pub` field on a domain | The macro's layer-1 check | designed |
| `ctx.orders()` for an undeclared domain | The Architecture Contract | **implemented** |
| `tokio::spawn(.. ctx ..)` | `Ctx<'req>`'s lifetime constraint | designed |
| Hand-written `impl Endpoint` / `impl Includes` | Sealed traits | **implemented** (`Includes`) |
| Returning `Ok(ctx)` from `when` | Preventing scope leakage | designed |

**The point is to prove there is no easier unchecked route around it**
([`../specs/unverified-boundaries.md`](../specs/unverified-boundaries.md)). If
even one case compiles, that is a hole in the spec.

### Update `.stderr` deliberately

```bash
TRYBUILD=overwrite cargo test
```

**Never run this without checking.** A change to error wording is a change to the
specification; read the diff.

- The wording improved → update and commit.
- The wording got worse → fix the implementation instead.
- It moved because of a rustc version difference → see §1.4.

### Handling drift between rustc versions

Errors involving cons lists and `There<There<..>>` drift easily between rustc
versions.

- Minimise the exposure with `#[diagnostic::do_not_recommend]`
  ([`type-level.md`](./type-level.md)).
- **UI tests are judged only in the CI job pinned to the MSRV.** Newer rustc runs
  in a separate job whose failures are advisory.
- If the drifting part cannot be removed, run the `.stderr` through a
  normalisation script that drops that line.

### How it is wired in CI (done in T-M0-03)

`.github/workflows/ci.yml` runs **one job per check** — `fmt`, `clippy`, `test`,
`msrv`, `docs`, `boundary`, `public-api`, `deny`, plus the `ci` aggregation gate.
Two of them concern the UI tests.

| Job | Toolchain | UI tests |
|---|---|---|
| `msrv` | **`1.85.0`, pinned to the patch** | **The authority.** Failures block |
| `test` | Latest stable | `continue-on-error`; shows as a red ✗ inside a green job |

Pinning to the patch matters because this job's premise is a deterministic
toolchain. Writing `1.85` resolves to the newest 1.85.x, and the standard could
move.

**Each check runs on exactly one toolchain.** `fmt` and `clippy` live only in the
stable job. Running the same check on two toolchains means that when rustfmt's
versions differ, **no formatting satisfies both at once** — and clippy would stop
unrelated PRs every time a new lint lands. Splitting the jobs makes the problem
not arise. **`.stderr` is the only thing that both drifts between versions and
needs pinning**, so it alone lives in `msrv`.

> **The clippy job runs with `--all-targets` (added in T-M0-12). The trade-off is
> worth recording.**
>
> It is needed so `disallowed_macros` ([logging.md](./logging.md)) **reaches test
> code**. Without it, clippy sees only the lib target, so **a `println!` in a
> committed test fails locally and passes in CI**.
>
> The cost is that the parenthesis above gets wider — **"clippy stops unrelated
> PRs every time a new lint lands" now extends to test code.** That is
> unavoidable while clippy runs on stable, and the judgement was that reliably
> stopping leftover debug output in tests is worth more. When a new stable lint
> turns test code red, **that is not a defect in the PR**; treat it like `.stderr`
> drift and decide on the spot whether to allow the lint or fix the code.

#### "Do not block on time-dependent checks" is not only about toolchains (second instance, T-M0-24)

The `deny` job (T-M0-24 / #24) uses no toolchain and made the same call.
`licenses`, `bans` and `sources` are determined by `Cargo.lock` alone, so they are
**deterministic and blocking**; `advisories` depends on the **upstream RUSTSEC
database, which updates independently of any PR**, so it is
`continue-on-error: true`.

The general form: **a check whose input lives outside the repository must not
block a PR.** For `.stderr` that external input is the rustc version, and the
answer was to pin one toolchain. For cargo-deny there is nothing to pin, so
non-blocking is the only option. Both come from the same reason — not turning
unrelated PRs red.

### Generate and judge `.stderr` at the MSRV locally too

**Running `cargo test --test ui` on the default toolchain fails.** The
repository's rustup override is 1.97.1, while `.stderr` is pinned to 1.85.0's
output.

```bash
cargo +1.85.0 test -p verum --test ui                    # judge
TRYBUILD=overwrite cargo +1.85.0 test -p verum --test ui # update (always read the diff)
```

**Forgetting `+1.85.0` while running `TRYBUILD=overwrite` rewrites `.stderr` with
a newer rustc's output and breaks the MSRV job.** That is a route to turning CI
red without touching the implementation, so watch for it.

### Measured drift (T-M1-04 / #16, 10 toolchains × 30 fixtures)

> An earlier version of this section recorded the **single-fixture era**
> (T-M0-06, a one-line diff). There are now 30 `compile_fail` and 4 `pass`
> fixtures covering E0277 / E0283 / E0119 / E0117 / E0603 / E0453 and six sealed
> traits. What follows is the re-measurement.

Re-run with `bash .github/scripts/measure-stderr-drift.sh` (`--no-dnr` for the
second arm). It is **an investigation tool, deliberately not wired into CI**, run
when raising the MSRV. Takes **about three minutes per arm**.

| Toolchain | As shipped | With `do_not_recommend` removed |
|---|---|---|
| 1.85.0 (baseline) / 1.89.0 / 1.90.0 | 0 files / 0 lines | 0 / 0 |
| **1.91.1** / 1.92.0 | **6 / 36** | **13 / 90** |
| **1.93.0** / 1.95.0 / 1.96.1 / 1.97.1 / nightly | 6 / 36 (**unchanged across 7 releases**) | **23 / 365** |

The `pass` fixtures are sound on every toolchain.

#### The drift is a single step, not churn

As shipped, it moves once at 1.91 and stays put for seven releases. That decides
the response directly: drift on every release would call for normalisation, but
once in seven makes normalisation **permanent maintenance debt for a rare
event**.

#### All six have the same shape, and Verum's wording did not move

rustc 1.91 changed the inline `= help: the trait X is not implemented for Y` into
a **`help:` block with a span pointing at the type definition**.

```text
- = help: the trait `verum::Index` is not implemented for `NotAnIndex`
+ help: the trait `verum::Index` is not implemented for `NotAnIndex`
+   --> tests/ui/compile_fail/index_bound_is_unsatisfied.rs:11:1
+ 11 | pub struct NotAnIndex;
+    | ^^^^^^^^^^^^^^^^^^^^^
```

**The diff in `on_unimplemented`'s message, label and note is zero across all
six.** What changed is rustc's own scaffolding and its span rendering — and it is
an **improvement**, since it now points at the definition of the offending type.

#### Which fixtures drift is predictable

The six that moved match exactly the fixtures **whose failing type is a local
nominal struct** (`MySet`, `MyList`, `Order` ×2, `NotAnIndex`, `MyIdx`). The 24
that did not fail on tuples, `()` or projections, which **have no definition span
for 1.91's improvement to point at**. Which class a new fixture falls into is
knowable before writing it.

#### The response: judge at the MSRV only, and add no normalisation filter

The criterion was fixed before measuring: *if the drift does not touch Verum's
own wording, judging at the MSRV alone is adequate. What the fixtures protect is
Verum's wording, and rustc reformatting its own scaffolding does not weaken
that.* The measurement did not touch it, so MSRV-only is adequate.

Three reasons not to normalise:

1. **The frequency does not justify it** — permanently maintaining "which lines
   to strip" for an event that occurs once in seven releases.
2. **What would be stripped is an improvement** — a `help:` block with a span
   carries more information for a human reader. Normalisation throws it away.
3. **The risk points the wrong way.** Getting "which lines to strip" wrong leaves
   `.stderr` guaranteeing nothing. `TRYBUILD=overwrite` as a reflex has already
   caused real damage in #7 and #9, and normalisation would be that reflex made
   into a mechanism.

#### `do_not_recommend` is a stability component, not a cosmetic one (updates RK-006)

Removing it takes the drift from 6 to **23** files and 36 to **365** lines — 3.8×
the files, 10× the lines — and it exposes **a 1.93 event that the shipped
configuration escapes entirely**.

1.93 turned `help: the following other types implement trait` into a span-carrying
block that **reproduces verum's own impl source lines**.

```text
+ help: the following other types implement trait `Has<T, Idx>`
+   --> src/typelevel.rs
+    | impl<H, T: ConsList> Has<H, Here> for (H, T) {}
+    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `(H, T)` implements `Has<H, Here>`
```

Without `do_not_recommend`, **`.stderr` would embed `typelevel.rs`'s impl
signatures, and changing one bound would churn ten or more fixtures.** That is
exactly the behaviour #9's review predicted for `has_duplicate_element.stderr`
(an E0283, which `do_not_recommend` cannot suppress), and those two fixtures
still expose impl signatures today — meaning **apart from two known exceptions,
`do_not_recommend` is what keeps the suite decoupled from bound changes.**

#### T-M0-03's CI split is correct (confirmed by measurement)

| Job | Toolchain | UI tests |
|---|---|---|
| `msrv` | 1.85.0, pinned to the patch | **The authority.** Blocking |
| `test` | Latest stable | Advisory, `continue-on-error: true` |

On stable (currently 1.97.1) the six mismatches above appear. That is **by
design**, and as the comment in `ci.yml` says, it is notice that the expected
output will need regenerating when the MSRV is raised — not a sign that the PR is
wrong. **No change proposed.**

> **A dev-dependency is bound by the MSRV too.** `trybuild` 1.0.120 declares
> `rust-version = 1.88`, which would mean **not one UI test runs** in the MSRV job
> (1.85.0). 1.0.119 is the last release supporting 1.85. `resolver = "3"` picks
> 1.0.119 automatically through MSRV awareness (confirmed to survive
> `cargo update`), but **the intuition that dev-dependencies are exempt from the
> MSRV is wrong.** If the UI tests are judged at the MSRV, their dependencies
> must satisfy it.

### The `ui` target is `test = false` (applied in T-M0-06)

```toml
[[test]]
name = "ui"
test = false   # keep it out of the bulk `cargo test`; CI invokes it with --test ui
```

**Without this, the `test` job's `cargo test --workspace` picks the UI tests up.**
That step **blocks**, so `.stderr` drift would stop PRs from the stable side and
the MSRV separation would be pointless. (The explicit invocation at the end of
the same job is `continue-on-error`, but it runs separately from this.)

`[[test]]` cannot be declared while `crates/verum/tests/ui.rs` does not exist —
manifest parsing errors out — so at T-M0-03 it was left as a note for T-M0-10.
**The UI test wiring was brought forward to T-M0-06**, so it is applied here. The
`hashFiles` guard on the CI side stays as it is; it is harmless.

---

## 2. Compile-success tests (`pass`)

Verify that a correct contract compiles as well.

```rust,ignore   // fragment, not a complete item
t.pass("tests/ui/pass/*.rs");
```

**With `compile_fail` alone, an implementation where everything fails to compile
passes the suite.** Always pair them.

---

## 3. Unit tests

`#[cfg(test)] mod tests` inside the source file. Subjects:

| Subject | Contents |
|---|---|
| Type-level parts | Whether `Has` / `Append` / `Lookup` resolve as expected — written so that matching types are what makes it compile |
| Cons-list generation | Flat input → cons list |
| Contract JSON | Whether the expansion produces the expected JSON |
| Route matching | Path parameter extraction, normalisation (`..`, encoded separators) |
| Body size limit, timeout | Boundary values |

### Naming

```text
<feature>_should_<expected_result>
```

```rust,ignore   // fragment, not a complete item
// ✅ states the behaviour
fn get_endpoint_should_have_empty_mutates()
fn append_should_dedup_duplicate_effects()
fn path_with_dotdot_should_be_rejected()
```

```rust,compile_fail
// ❌ states the implementation
fn test_append()
```

---

## 4. Integration tests

They live in `crates/verum/tests/`. Define real endpoints and send requests
through them.

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
#[tokio::test]
async fn get_user_should_return_declared_fields_only() { }
```

### Building a `Ctx` goes through the test API only

**Do not expose a god-mode constructor**
([`api-surface.md`](./api-surface.md)).

```rust,ignore   // fragment, not a complete item
// ✅ the user cannot choose the endpoint type freely
verum::test::run::<GetUser>(req, mocks).await
```

```rust,compile_fail
// ❌ lets a Ctx be built for any endpoint type
Ctx::for_test(deps)
```

Making `Ctx::for_test` `pub` lets a user define their own `impl Endpoint` and
build a `Ctx` holding every capability. `#[cfg(test)]` does not cross a crate
boundary, so a `test-util` feature enabled transitively ships in the production
binary.

> **The test strategy is a gap in the current design**
> ([`../specs/research-questions.md`](../specs/research-questions.md)). The shape
> of `verum::test::run` is decided during the First PoC.

---

## 5. Snapshot tests for the contract output

The AI Context JSON is a specification. Pin it with a snapshot.

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
#[test]
fn contract_json_should_include_enforcement_and_boundaries() {
    let json = contract_of::<UpdateUser>();
    insta::assert_json_snapshot!(json);
}
```

**Fields whose presence must be verified**
([`../specs/ai-context.md`](../specs/ai-context.md)):

- `enforcement` (`upper_bound_checked` / `intent_only` / `metadata_only` / `none`)
- `unverified_boundaries`
- `scope_of_readonly_guarantee`
- `condition_verified: false`
- `escape_hatches` — `"unknown"`, not `[]`

**Fail the test if the value `type_checked` ever appears.** The contract is an
upper-bound check, not a bidirectional one.

---

## 6. Measuring compile time

Type-level computation feeds straight into compile time. **Treat it as a kind of
performance test.**

```bash
cargo build --timings
```

Measure how it scales as endpoints go from 1 to 10 to 50. If it exceeds 2× Axum,
narrow the scope of the type-level computation
([`../specs/evaluation.md`](../specs/evaluation.md), kill criteria).

Not run in CI. Run it by hand before and after changing the type design.

---

## 7. AI coding benchmarks

**Separate from ordinary testing.** The method, metrics and kill criteria are
defined in [`../specs/evaluation.md`](../specs/evaluation.md).

- Not run in CI — non-deterministic and expensive.
- Run before and after a substantial change to the type design or the error
  messages.
- **Always collect the points where the AI hesitated** — the Q-C experiment found
  **three holes in the spec** that way.

---

## 8. Benchmarks (Criterion)

Only the runtime's critical path.

| Subject | Contents |
|---|---|
| Route matching | Path resolution |
| Request → endpoint dispatch | The erasure layer's overhead |
| Throughput relative to Axum | The same endpoint on both |

Not run in CI. Run by hand before and after a change that touches performance.

---

## 9. Rules for verification harnesses

This section governs **verification harnesses**: `compile_fail` fixtures, CI
guards, `.github/scripts/*.sh`, `spikes/*/run.sh`. It is written as *rules*
rather than guidance because the same class of hole opened **seven times across
five PRs** — #9 (three in a row), #12, #16, #24, #13 — and **three more times
inside #13's own fix pass** (rule 4's mechanism not reaching integration tests,
rule 5's `touch` not recursing, rule 13 missing entirely), so ten in total. The
mistake each time was fixing the instance instead of the class.

**The whole set folds into one line: assert that the work happened, not that no
error appeared.**

| # | Rule | What happens otherwise (all measured) |
|---|---|---|
| 1 | **A rejection must carry its expected error code.** Do not judge on exit status alone | Something that failed for an unrelated reason — a typo, a missing import — is counted as "correctly rejected". Two predictions were caught by this rule in #13 |
| 2 | **A pass marker asserts a count** | `cargo test` runs three targets and two always print `test result: ok. 0 passed`, so `test result: ok` matches even with the test file deleted. **Limits**: a count is not an identity, so a swap goes undetected; adding an unrelated test is a false positive; `--test <target>` narrows the scope and localises the number |
| 3 | **A pass marker must not depend on build-cache state** | `Checking <crate>` only appears on a cold build. Using it as the needle turns unrelated lines red on the second run, and reveals that **the first perfect score depended on cache state**. `Finished` appears warm or cold |
| 4 | **Assert that the probe's code was actually compiled** | A mismatched `#[cfg(feature = "…")]` name means the code never compiles at all, `Finished` appears, and it counts as a pass. **Set `[lints.rust] unexpected_cfgs = "deny"` per package in `Cargo.toml`** — a `#![deny(...)]` in `lib.rs` **does not reach that package's integration-test crates** (they are separate crates; measured, two probe hosts were uncovered). A blanket `RUSTFLAGS` is not an option since it judges dependencies too |
| 5 | **Run the baseline self-check after invalidating the cache** | Breaking only external state — a database schema, an environment variable — without touching a source file lets cargo replay a cached diagnostic, producing **a table of mixed green and red**. That is worse than all red: the green rows read as "the conclusions still hold" |
| 6 | **When a destructive probe passes, suspect the probe first** | Confirm that what you planted was what you meant. In #24 the unused-licence probe was wrong and the configuration was right |
| 7 | **Everywhere a set of files is enumerated**, recurse into directories, derive the target names from the declaration side, and forbid aliases | #9 had the same scoping hole three times running, and a non-recursive scan let a forgery of ledger path 14 pass green. **This is not limited to scanning guards** — the `touch app/src/*.rs` implementing rule 5 used a non-recursive glob, so creating one subdirectory would silently disarm the baseline self-check (a recurrence inside this very rule set) |
| 8 | **Another tool's settings do not necessarily do what their names suggest** | cargo-deny's `[graph] exclude-dev` changes nothing when set. What works is `[licenses] include-dev`, and without it a GPL dev-dependency passes straight through |
| 9 | If a construct that swallows failures (`\|\| true`, a pipe, process substitution) sits on the judging path, **assert separately that the work happened** | A missing toolchain was reported as "no drift", and the same mechanism disabled the baseline self-check too. **This is not a ban** — the `\|\| true` in `measure-stderr-drift.sh` is deliberate (trybuild always exits non-zero when expectations move) and compensates by confirming the `ui-*` binaries were produced; `check-api-boundary.sh`'s grep no-match is compensated by `scanned -eq 0`. Writing it as a ban would fail those two **correct** implementations |
| 10 | **Confirm the restore mechanism works before trusting the results** | In a scratch copy without `.git`, `git checkout --` silently does nothing. Probes accumulated and two paths were nearly misread as leaking. Make a fresh copy per mutation and **assert before running that the copy matches the original** (`diff -r -q`) — copying alone is not enough (a concurrent process once destroyed one with `rsync --delete`) |
| 11 | **The baseline self-check covers every crate that hosts a probe** | The gate ran `-p app` only, and `separate-repo`'s six probes were never confirmed in their unmutated state |
| 12 | **Assert that the claimed environment is the one measured. Printing is a claim, not an assertion** | A `rustup override` does not travel with the tree, so a copy outside the repository measures a different rustc. A reviewer actually measured nightly (the verdict happened to be the same on both, which was luck) |
| 13 | **A pass probe asserts that the code still exists and still goes through the mechanism** | Rule 4 only says "it compiled". A pass probe stays green with its body emptied (measured on 8 of 10). Put a type-level assertion (`const _: fn(A) -> B = f;`, or a trait bound) **at the call site** — inside a macro's output it disappears with the macro (measured). **Remaining limit**: emptying a function body is not catchable at the type level |
| 14 | **A rejection probe must also be checked by removing the cause it names** | An error code can be right while the cause is wrong. A probe expecting `E0038` from a `Ctx` parameter kept producing `E0038` with the parameter removed — it was the `Sized` supertrait all along, and the probe had never measured what it claimed. This is rule 6 in the other direction: **when a rejection probe fails, suspect the probe too** |

**And do not mistake what this is.** Text scanning cannot detect a false note.
**The defence proper is the `compile_fail` / `pass` fixture pair**; a scanning
guard only makes someone write the claim down.

---

## What not to test

- Third-party internals (tokio, hyper, tower, axum, sqlx).
- **The exact text of generated code.** `cargo expand` snapshots are brittle;
  prefer UI tests, which cover the error side.
- Getters with no logic.
- **"Is it implemented according to the contract?"** — that is the compiler's
  job. What tests check is that *the compiler rejects things*.

---

## Test helpers

Shared utilities live in `crates/verum/tests/common/`.

- Mock repositories with fixed responses.
- Test domains (`TestUser` and similar) reused across UI tests.
- A wrapper around `verum::test::run`.

**Keep UI-test fixtures minimal.** Unrelated types in the error output make
`.stderr` hard to read.
