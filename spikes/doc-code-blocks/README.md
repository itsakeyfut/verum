# #43 / #38 — does every published code block and JSON sample hold up?

`docs/` carries 211 fenced Rust blocks. Before this harness, **none of them was
checked**: a block could contradict the prose beside it, reference a type that is
declared nowhere, or claim "this does not compile" about something that does,
and nothing would notice. #43 exists because a whole-design review found twelve
such contradictions by reading — and because #14 then built an entire verdict on
a signature it had transcribed from one of these blocks incorrectly.

```bash
bash run.sh          # classify, compile, and assert the remainder has not grown
```

Measured on **rustc 1.85.0** (Verum's MSRV).

---

## What it does

| Script | Role |
|---|---|
| `extract.py` | finds every ` ```rust ` block; refuses an unterminated fence or an empty scan |
| `check.py` | compiles each block **on its own**, so one parse error does not hide 200 results |
| `classify.py` | proposes a fence tag; everything it cannot decide is reported, not guessed |
| `split_mixed.py` | splits fences that carry a ✅ form and a ❌ form together |
| `apply_tags.py` | writes the decided tags back into the documents |
| `stub/` | the designed-but-unimplemented surface, **every item citing the document line it comes from** |
| `check_json.py` | validates every ` ```json ` block against the one AI Context schema (#38 / ADR-0008) |

Tags follow rustdoc, so GitHub renders the documents unchanged:

```text
```rust                 must compile
```rust,compile_fail    must NOT compile
```rust,ignore   // …   not checked, and the fence says why
```

## Current state

```
228 blocks
  compile_fail   54    every one verified to actually fail, every run
  ignore        139    fragment / needs an M2 macro / needs an absent crate /
                       verum-internal / body elided / module without its imports
  ok             35
  remaining       0

16 json samples, 18 ledger kinds, 0 violations
```

Both directions are guarded, and both were demonstrated by breaking them:

* an untagged block that fails → `FATAL: 1 unmarked blocks fail, expected at most 0`
* a `compile_fail` block that starts compiling → `FATAL: 1 block(s) tagged compile_fail now compile`

The second guard did not exist until it was tested for. `run.sh` checked only the
first, so replacing a forgery example with `struct Trivial;` printed
`compile_fail:BAD 1` and still exited 0.

## The JSON half (#38)

The AI Context schema is written out in **six files that must agree key for key**,
and the Rust harness never looks at a ` ```json ` fence. `check_json.py` closes
that: it parses every JSON sample under `docs/` — tolerating object fragments and
`...` elisions — and enforces what ADR-0008 decided.

The load-bearing rule is the **join**: `enforcement.voided_by` may only name a
`kind` that the ledger actually emits, and the kind list is *parsed out of
`unverified-boundaries.md`'s own sample* rather than hard-coded. Neither side can
drift alone.

The other load-bearing rule is that **a key claiming a guarantee carries an
`enforcement` at all** — `fields` / `domains` / `emits` / `calls`. Checking the
*shape* of an enforcement that is present misses the case where there is none,
which is the case that actually happened, twice. The rule is ancestor-aware: a
nested breakdown such as `mutates.conditional` inherits the enforcement on
`mutates` and is not asked to repeat it.

Every rule was broken on purpose and observed to exit 1 before being adopted —
including the two that report *absence* (`no json fences found`, `no kind values
found`), because a scan that finds nothing must not read as a pass, and including
`run.sh` itself, to confirm the failure propagates rather than being swallowed.

**It found three real defects that reading had not.** Two on its first run, both
in `effect-inference.md` — a scalar `enforcement` and a `deferred: []`. The third
in code review: the top-level `conditional[]` entry in `conditional-effects.md`
had no `enforcement` while `ai-context.md`'s had one, so two samples of one
schema disagreed inside the change that exists to stop exactly that. All three
would have shipped.

## What it found

- **`Repo`, `Runtime` and `Field` are used but declared nowhere** in `docs/`.
  Seven or more blocks depend on them. Found while writing the stub — there was
  nothing to copy. Each now has a `proposed` ADR.
- **`When` was wrongly listed with them.** It *is* declared, in
  `rust-type-model.md:73` and `type-level.md:324`, and the two agree. The stub
  lacked it, so the harness failed, and the failure was written down as a
  documentation defect. **A red run means "the docs are wrong" *or* "the stub is
  behind", and this one was the latter** — exactly the ambiguity the ADR warns
  about, walked into by the person who wrote the warning.
- **`Handler` does not compile as printed** (`rust-type-model.md:327`,
  `rules/rust.md:76`): it names `Self::Request` with no supertrait that declares
  it. `type-level.md:412` has the same shape for `Self::R`.
- **`Condition` is implemented with the wrong arity**
  (`unverified-boundaries.md:176` passes one argument to a two-parameter trait).
  The correct form is in `conditional-effects.md:233` — the documents settle this
  between themselves, which is why it is a fix and not a judgement call.
- **24 blocks put a ✅ example and a ❌ example in one fence.** No tag is true of
  such a block, so both halves went unchecked. Split into 48 halves: all 26 of
  the ❌ halves do fail, as claimed.
- **4 blocks contain no code at all** — prose inside a Rust fence, which reads as
  checked and checks nothing.
- **`Owner` is used in two documents and declared in none**
  (`type-level.md:416`, `architecture-contract.md:51`). The associated type it
  needs is now declared; **what it means is still undecided** and is left marked
  as such rather than invented.
- **Two documents describe a bypass that no longer works.** `impl Includes<Order>
  for User {}` is introduced with "orphan rule を通り、`cargo build` は成功し" —
  it has not compiled since `Includes` was sealed in #6. Now tagged
  `compile_fail`, so the seal is re-verified on every run.
- **`Handler` and `CtxUsers` were missing associated types** they name
  (`Self::Request`, `Self::R`, `Self::M`). Fixed from the declarations the other
  copies in `docs/` already carried — the documents settled it between
  themselves, which is why these are corrections and not judgement calls.

## The stub cites its sources, deliberately

Every item in `stub/src/lib.rs` names the document line it was copied from:

```rust
/// `docs/specs/rust-type-model.md:48`. The seal is dropped: `SealedEndpoint`
/// does not exist yet …
pub trait Endpoint { … }
```

#14 failed because a signature was **re-derived from a reading** of the specs
rather than transcribed from them, and the re-derivation bound three
higher-ranked lifetimes into one. A stub written from memory would bake the same
class of error into every block it checks. When a block fails against this stub,
the fix is either the block or the spec — never a quiet adjustment here.

## The harness was wrong six times

Each was found by suspecting the harness rather than the documents
(`docs/rules/test.md` §9-6):

| Hole | How it surfaced |
|---|---|
| `#[test]` bodies were never type-checked (missing `--test`) | a block calling two undefined functions was scored as passing |
| `extract.py` resolved `docs/` relative to the working directory | the empty-scan `FATAL` fired |
| the ❌ heuristic read three lines of prose above the fence | all seven hits it produced were false positives |
| mixed ✅/❌ blocks were collapsed to `compile_fail` | adding the mixed check moved 25 blocks |
| fence length was not matched, so a ```` ```text ```` block *showing* fence syntax had its contents read as fences | `docs/adr/0003` — the document describing this harness — reported a `compile_fail` block that compiles |
| the exit status ignored `compile_fail:BAD` entirely | a `compile_fail` block replaced with `struct Trivial;` printed the failure and exited 0 |

The first is `docs/rules/test.md` §9-4 exactly — the rule this repository wrote
after the same class opened ten times.

## Limits

- **Not a CI guard yet.** It depends on a stub standing in for a framework
  surface that does not exist, so a red run can mean "the docs are wrong" *or*
  "the stub is behind". Worth wiring in once M2 lands the real macros and the
  stub shrinks.
- `ignore:frag` is inferred from rustc's parse errors. A block that fails to
  parse *because it is wrong* is indistinguishable from one that is merely a
  fragment.
- The 23 remaining blocks are unclassified, not verified. `EXPECTED_BAD` in
  `run.sh` keeps that number from growing; lowering it is the work.
