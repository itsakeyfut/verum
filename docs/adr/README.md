# Architecture Decision Records

**A design decision's rationale lives here and nowhere else.** `docs/specs/**`
states the outcome in one sentence and links to the record; it does not repeat
the reasoning.

These are written in **English**, unlike the rest of `docs/` — same reason as
`CLAUDE.md` and the slash commands. Why the directory exists at all, and what
belongs in it, is [ADR-0000](./0000-record-architecture-decisions.md).

Format: [MADR 4.0](https://adr.github.io/madr/). Copy
[`adr-template.md`](./adr-template.md) to start one.

---

## Index

| # | Decision | Status | Confirmed by |
|---|---|---|---|
| [0000](./0000-record-architecture-decisions.md) | Record architecture decisions as MADR, in English, as the single home for rationale | accepted | `spikes/doc-code-blocks`, plus a grep rule in `CLAUDE.md` |
| [0001](./0001-includes-is-implemented-on-the-endpoint.md) | Implement `Includes<D>` on the endpoint type, not on the domain set | accepted | UI fixture + satisfiability test in `crates/verum` |
| [0002](./0002-ctxusers-exposes-the-endpoint-as-owner.md) | Expose the endpoint type as an `Owner` associated type | accepted | **nothing yet** — needs a `compile_fail` fixture in M3 |
| [0003](./0003-doc-code-block-tags.md) | Declare whether a documentation code block is checked, using rustdoc fence tags | accepted | `spikes/doc-code-blocks/run.sh` |
| [0004](./0004-reads-enforcement-level.md) | Whether capability-checked getters enforce `reads`, or `reads` stays metadata only | **proposed** | **nothing** — waits on #15 |
| [0005](./0005-repo-handle-shape.md) | What `Repo<D, R, M>` is, and whether it carries the request lifetime | **proposed** | `spikes/ctx-lifetime-rpitit` E1–E4b — the escape is asserted at run time and **both** candidates block it; the decision waits on #39 |
| [0006](./0006-runtime-sealed-token.md) | What `Runtime<Sealed>` is, and whether a sealed token closes the god-mode constructor | **proposed** | `spikes/ctx-lifetime-rpitit` A0 — visibility blocks *construction* only; a public erased-handler entry point still supplies a live `Ctx` |
| [0007](./0007-field-trait-shape.md) | What `Field<D>` declares, and what forging it would buy | **proposed** | **nothing** — the seal has no trait to match |
| [0008](./0008-guarantees-carry-scope-and-voiding-paths.md) | Every AI Context key that claims a guarantee carries its scope and the paths that void it | accepted | `spikes/doc-code-blocks/check_json.py` — plus an adversarial re-read that no checker can replace |

**By status** — proposed: **0004, 0005, 0006, 0007** · accepted: 0000–0003, 0008 · superseded: none

> **Four `proposed` records, and the codebase relies on all four.** That is the
> state [ADR-0000](./0000-record-architecture-decisions.md) calls a defect, made
> visible rather than fixed. Each names the issue that settles it: #15, #39
> (with #40), path 9, path 14.
>
> **0005 and 0006 now have measurements** (T-M1-02 / #14) without having
> decisions. That is the intended order — the spike measures, the issue decides —
> but it also surfaced that **#39 and #40 cannot be decided independently**, and
> that 0006's question is larger than it was written: blocking the *constructor*
> does not block a *supplier*.

## Conventions

* Filename `NNNN-short-slug.md`, numbers consecutive, no gaps.
* YAML front matter carries `status`, `date`, `decision-makers`, and
  `enforcement-level` where one applies.
* **Every record fills in *Confirmation*.** If nothing would fail when the
  decision is violated, say "nothing yet" — as ADR-0002 does. A decision that
  looks enforced and is not is the failure this repository keeps repeating.
* Reversing a decision means a new record, with the old one marked
  `superseded by ADR-NNNN` and the measurement that reversed it written down.
* A name used in `docs/` but declared nowhere gets a record as `proposed`, rather
  than a meaning inferred by whoever reads it next.
