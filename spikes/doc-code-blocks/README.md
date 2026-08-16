# #43 — does every Rust code block in `docs/` compile?

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

Tags follow rustdoc, so GitHub renders the documents unchanged:

```text
```rust                 must compile
```rust,compile_fail    must NOT compile
```rust,ignore   // …   not checked, and the fence says why
```

## Current state

```
211 blocks
  compile_fail   49    every one verified to actually fail
  ignore        114    fragment 68 / needs a macro from M2 26 / needs an absent crate 20
  ok             25
  text            4    no code at all — prose in a Rust fence
  remaining      23    still unclassified; `run.sh` asserts this does not grow
```

## What it found

- **`Repo`, `Runtime`, `Field` and `When` are used but declared nowhere** in
  `docs/`. Seven or more blocks depend on them. Found while writing the stub —
  there was nothing to copy.
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

## The harness was wrong four times

Each was found by suspecting the harness rather than the documents
(`docs/rules/test.md` §9-6):

| Hole | How it surfaced |
|---|---|
| `#[test]` bodies were never type-checked (missing `--test`) | a block calling two undefined functions was scored as passing |
| `extract.py` resolved `docs/` relative to the working directory | the empty-scan `FATAL` fired |
| the ❌ heuristic read three lines of prose above the fence | all seven hits it produced were false positives |
| mixed ✅/❌ blocks were collapsed to `compile_fail` | adding the mixed check moved 25 blocks |

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
