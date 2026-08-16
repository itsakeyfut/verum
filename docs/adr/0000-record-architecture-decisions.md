---
status: accepted
date: 2026-08-16
decision-makers: itsakeyfut
---

# Record architecture decisions as MADR, in English, as the single home for rationale

## Context and Problem Statement

This project already has decision logs. Seven files under `docs/specs/**` carry a
「却下した案」(rejected alternatives) section, and `docs/dev/*/review-knowledge.md`
records footguns. What is missing is not a log — it is the guarantee that a
decision exists in exactly one place.

Three failure modes have been measured, all of them within a single week:

1. **A claim is copied, then corrected in only some copies.** "`when` requires an
   async closure" was duplicated across **eight documents**. When T-M1-02 (#14)
   found it wrong, the correction reached five and missed three — and the
   correction was itself wrong, because it had been derived from a code block
   nobody had ever compiled.
2. **Undecided things are used as if decided.** `CtxUsers::Owner` appeared in two
   documents as `where Self::Owner: Includes<User>` and was **declared in none**.
   `Repo`, `Runtime`, `Field` and `When` are the same: usage without declaration.
   A reader cannot tell "nobody decided this" from "I missed where it was
   decided".
3. **A rejection outlives the reason for it.** An alternative rejected under one
   set of constraints stays rejected after those constraints change, because the
   rejection is recorded and its grounds are not.

(2) is the immediate trigger. **There was nowhere to write down that something
had not been decided.**

## Decision Drivers

* A decision's rationale must live in exactly one file, or (1) recurs.
* "Not decided" must be expressible, or (2) recurs.
* The format must be one an outside contributor already knows.
* The record must say how compliance is checked — this repository has repeatedly
  shipped claims with no test behind them.

## Considered Options

* **MADR 4.0** (<https://adr.github.io/madr/>) — the de facto Markdown ADR
  standard; YAML front matter, explicit *Considered Options*, and a
  *Confirmation* section.
* **Nygard-style ADR** — Context / Decision / Consequences. What the AWS
  prescriptive-guidance article describes.
* **Keep rationale in `docs/specs/**` and add discipline** — no new directory.

## Decision Outcome

Chosen option: **MADR 4.0, written in English**, under `docs/adr/`.

`docs/specs/**` states the *outcome* of a decision in one sentence and links
here. **The rationale is not duplicated.** That single rule is what addresses
failure mode (1); the directory on its own would not.

### Language

Everything published — `docs/adr/`, `docs/rules/`, `docs/specs/` and
`docs/concepts.md` — is in **English**. These documents constrain implementation,
and both contributors and tooling read English instructions more reliably.

The internal directories `docs/dev/` and `docs/roadmap/` stay Japanese and stay
private. Nothing published links into them: where a published document needs a
fact recorded there, the fact is written out on the published side rather than
linked.

### Where each kind of writing belongs

| Location | Holds | Does not hold |
|---|---|---|
| `docs/specs/**` | **what the design is** — shapes, guarantees, signatures | why it was chosen; links here instead |
| `docs/adr/**` | **why it was chosen, when, and what would reverse it** | type detail; links to the specs |
| `docs/rules/**` | what to do while implementing | how a decision was reached |
| `docs/dev/*/review-knowledge.md` | footguns hit, and verified non-issues | design decisions |
| `docs/roadmap/**` | what to do next | how a decision was reached |

### Status vocabulary

MADR's statuses are used unchanged: `proposed`, `rejected`, `accepted`,
`deprecated`, `superseded by ADR-NNNN`.

> **`proposed` while the codebase already relies on the decision is itself a
> defect.** That is the shape `Owner` was in: used in two documents, decided in
> none. When an ADR is `proposed` and something already depends on it, say so in
> *Context and Problem Statement* — do not let the status be read as "work in
> progress on something nobody uses yet".

### Confirmation

Every ADR fills in its own **Confirmation** section: which fixture, guard or
harness would fail if the decision were violated. **If nothing would fail, the
section says so.** This repository's recurring defect is a claim with no test
behind it, so an empty Confirmation is a finding, not an omission.

Two mechanical checks back this ADR itself:

* `spikes/doc-code-blocks/run.sh` compiles the code in these files, so an ADR
  cannot describe a signature that does not exist. It is how `Owner` was found.
* After correcting a claim, grep the whole repository for the **old** wording.
  Recorded as skipped three times; now a rule in `CLAUDE.md`.

### Consequences

* Good, because a reader looking for "why" has exactly one place to look.
* Good, because `proposed` makes open questions visible instead of invisible.
* Good, because MADR is a format contributors have seen elsewhere.
* Bad, because there is now a fifth place documentation can live. The boundary
  table above is what keeps it from becoming a duplicate of the specs; if ADRs
  are observed duplicating spec content twice, redraw the boundary.
* `docs/specs/research-questions.md` overlaps with `proposed` ADRs. It keeps only
  the questions that have **not** yet become ADRs; once one does, it moves.
* The 「却下した案」 sections in the specs move here **as they are touched**, not
  in one sweep.

## When to write one

* **A name is used in `docs/` and declared nowhere** — open one as `proposed`.
  `spikes/doc-code-blocks` finds these mechanically.
* Two or more implementations are possible and one is chosen.
* An existing decision is reversed — write a new ADR, mark the old one
  `superseded by ADR-NNNN`, and say what measurement reversed it.
* You are about to write 未決 or 未検証 into a spec.

**Not worth an ADR:** naming, formatting, anything affecting one call site.

## More Information

* [MADR 4.0](https://adr.github.io/madr/) — the template this follows
* [`adr-template.md`](./adr-template.md) — copy this to start a new one
* `docs/dev/maintenance-tasks.md`, entry dated 2026-08-16 — the review that
  produced this ADR, including the eight-document propagation
* `docs/rules/README.md` §Rules specific to Verum — the same discipline applied while implementing
