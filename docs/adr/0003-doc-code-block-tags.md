---
status: accepted
date: 2026-08-16
decision-makers: itsakeyfut
enforcement-level: none
---

# Declare whether a documentation code block is checked, using rustdoc fence tags

## Context and Problem Statement

`docs/` carries over two hundred fenced Rust blocks and **not one of them was
compiled.** What that allowed, all of it measured during #43:

* **`Handler` does not compile as printed** — it names `Self::Request` with no
  supertrait declaring it. The same declaration appears in two documents; both
  were broken.
* **`Includes` names the wrong subject in five places**
  ([ADR-0001](./0001-includes-is-implemented-on-the-endpoint.md)).
* **`Owner`, `Repo`, `Runtime` and `Field` are used and never declared.**
  (An earlier version of this list also named `When`. That was wrong — it *is*
  declared, consistently, in `rust-type-model.md:73` and `type-level.md:324`. The
  stub simply lacked it, and the harness's failure was read as a documentation
  defect rather than a stub defect. Recorded because it is the same class of
  error the harness exists to catch, committed by its author.)
* **A closed bypass is documented as open.** `impl Includes<Order> for User {}`
  has been rejected since `Includes` was sealed in #6, yet two documents still
  said "the orphan rule permits it, `cargo build` succeeds" in the present tense.
* **24 blocks put a ✅ example and a ❌ example inside one fence.** No tag is true
  of such a block, so neither half was ever checked.
* **4 blocks contained no code at all** — prose inside a Rust fence, which reads
  as checked and checks nothing.

Then T-M1-02 (#14) transcribed a signature from one of these unchecked blocks,
got it wrong, and built a verdict, a new "canonical" spec section, an inverted
knowledge-bank entry and eleven document rewrites on top of the error.

## Decision Drivers

* An AI or a contributor copying from the docs copies the **code block**, not the
  note beside it (`docs/specs/handler-rules.md:174`).
* Most blocks reference types that do not exist yet, so "make everything compile"
  is not available.
* Whatever is not checked must say so, and say why, at the point of use.

## Considered Options

* **rustdoc fence tags** — `rust` / `rust,compile_fail` / `rust,ignore`
* **Opt in to checking** — bare fences unchecked, `rust,check` to check
* **A separate manifest** listing which blocks are checked
* **rustdoc doctests** — move the blocks into crate documentation

## Decision Outcome

Chosen option: **rustdoc fence tags**, with checking as the default.

````text
```rust                  must compile
```rust,compile_fail     must NOT compile
```rust,ignore   // why  not checked; the reason sits on the fence line
```text                  not code — prose, or compiler output
````

GitHub renders these unchanged, and the vocabulary leaves the door open to
`cargo test --doc` if parts of the specs later move into rustdoc.

**The reason for an `ignore` goes on the fence line.** Put it in a table and the
table drifts away from the block — which is the exact failure this ADR exists to
end.

### The machine proposes; a person decides

`spikes/doc-code-blocks/` extracts, compiles and classifies. Four categories are
mechanical — fragment, needs an M2 macro, needs an absent crate, contains no code
— and **anything else is reported as `REVIEW` rather than guessed.**

### Confirmation

`spikes/doc-code-blocks/run.sh` fails in **both** directions, and both were
demonstrated by breaking them:

* an untagged block that does not compile — `FATAL: 1 unmarked blocks fail`
* a `compile_fail` block that starts compiling — `FATAL: 1 block(s) tagged
  compile_fail now compile`

The second did not exist until it was tested for. Without it a claim could go
false in silence, which is the failure this ADR is about.

**It is deliberately not wired into CI.** A red run is ambiguous today: it can
mean "the documentation is wrong" or "the stub has not caught up with the
implementation". Revisit once M2 lands the real macros and the stub shrinks.

### Consequences

* Good, because a block that says "this does not compile" is now proved to not
  compile — 54 of them, every run.
* Good, because the seal added in #6 is re-verified by the documentation that
  describes it.
* Bad, because a stub crate is required for the types that do not exist yet.
  Every item in it cites the document line it was copied from: #14 failed by
  **re-deriving** a signature from a reading, so the stub must be a transcription,
  never a reconstruction.
* `ignore:frag` is inferred from rustc's parse errors, so "a fragment" and "wrong
  in a way that fails to parse" are indistinguishable.
* Counts on 2026-08-16, including this ADR set: `compile_fail` 54, `ignore` 128,
  `ok` 33, unclassified 0.
* **Both directions are guarded.** An untagged block that fails is fatal, and so
  is a `compile_fail` block that starts compiling — the second check was missing
  until it was tested for, and without it a claim could go false silently.
* **This ADR found a bug in its own harness.** Showing the tag vocabulary inside a
  ` ```text ` block made the extractor read the inner ` ```rust ` lines as real
  fences, and it reported a `compile_fail` block that compiles. Fence lengths were
  not being matched; the outer fence is now ```` and the extractor compares
  backtick counts.

## Pros and Cons of the Options

### rustdoc fence tags

* Good, because the default is the safe one: a new block is checked unless
  someone says otherwise.
* Good, because the vocabulary is already known to Rust readers.
* Bad, because 118 blocks needed a tag added.

### Opt in to checking

* Good, because far fewer edits.
* **Bad, because the default is the unsafe one.** New blocks arrive untagged and
  therefore unchecked, so the unchecked set grows forever.

### A separate manifest

* **Bad, because the manifest and the blocks drift apart** — the failure mode
  this whole ADR is about.

### rustdoc doctests

* Good, because no bespoke harness.
* **Bad, because `docs/` is standalone Markdown, not crate documentation.**
  `cargo test --doc` never looks at it. Worth reconsidering if the specs move
  into rustdoc after M2.

## More Information

* `spikes/doc-code-blocks/README.md` — the harness, and the six times it was wrong
* `docs/rules/test.md` §9 — the rules for verification harnesses; this one broke
  §9-4 (`#[test]` bodies were never type-checked) before it caught anything else
* `docs/dev/maintenance-tasks.md`, entry dated 2026-08-16 — the full account of #14
