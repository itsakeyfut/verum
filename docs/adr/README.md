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
| [0004](./0004-reads-enforcement-level.md) | Whether capability-checked getters enforce `reads`, or `reads` stays metadata only | proposed | `spikes/reads-getter-enforcement` — the getters enforce (`E0277`), but only in a derive-emitted extension trait and on two undesigned preconditions; the level stays `metadata_only` |
| [0005](./0005-repo-handle-shape.md) | `Repo<'req, D, R, M>` — the capability handle carries the request lifetime | accepted | `spikes/ctx-lifetime-rpitit` E1–E4b (the escape asserted at run time; both candidates block it) + `tests/ui/compile_fail/repo_handle_cannot_outlive_its_request.rs` (`E0521`) and its `pass` pair. ⚠️ The fixture pins the hand-written type; **the producer side is unpinned and is #40's** |
| [0006](./0006-runtime-sealed-token.md) | What `Runtime<Sealed>` is, and whether a sealed token closes the god-mode constructor | **proposed** | `spikes/ctx-lifetime-rpitit` A0 — visibility blocks *construction* only; a public erased-handler entry point still supplies a live `Ctx` |
| [0007](./0007-field-trait-shape.md) | What `Field<D>` declares, and what forging it would buy | **proposed** | **nothing** — the seal has no trait to match |
| [0008](./0008-guarantees-carry-scope-and-voiding-paths.md) | Every AI Context key that claims a guarantee carries its scope and the paths that void it | accepted | `spikes/doc-code-blocks/check_json.py` — plus an adversarial re-read that no checker can replace |
| [0009](./0009-observed-is-not-a-lower-bound.md) | `observed` is not a lower bound, and Q-A is reopened on the measurement | accepted | `spikes/contract-from-tokens/` — probes **V1** and **V2**, which a sound or complete scanner would turn red. ⚠️ The key it names was renamed to `syntactically_present` by [ADR-0014](./0014-syntactically-present-replaces-observed.md) — this row keeps `observed` because that was the name when the reopening was decided |
| [0010](./0010-domain-constructor-confined-by-module-privacy.md) | The Domain constructor is confined to a macro-owned module, not to the user's | accepted | `spikes/domain-opacity-sqlx/run.sh` — **P31** (`E0624`), **P32** (crate root), **P34** (`as_repr`), **P35** (the `Repr`); P31 mutation-verified. **P33 / P36 are the counter-evidence rows and must pass** |
| [0011](./0011-domain-is-an-attribute-macro.md) | The Domain macro is an attribute, `#[domain]`, not `#[derive(Domain)]` | accepted | `spikes/domain-opacity-sqlx/run.sh` — **P38** (`E0255`, a derive cannot emit ADR-0010's shape), **P39** (an attribute can), **P39b** (`E0624`, the confinement survives). Plus the widened `imports` guard, planted and confirmed red |
| [0012](./0012-spawn-takes-a-payload-not-a-context.md) | `ctx.spawn` takes a **payload**, not a context | accepted | `spikes/ctx-lifetime-rpitit` F1–F6 — the specified shape is `E0521`, the owned form compiles but can be re-spawned, and the chosen form is `E0521` against both re-spawning and payload smuggling. ⚠️ No fixture in `crates/verum` yet — there is no `Ctx` there; relocated to #60 / `T-M3-02` |
| [0013](./0013-includes-is-a-blanket-impl.md) | `Includes` is a blanket impl, so its seal never becomes derive-facing | accepted | `spikes/seal-after-m2` S1–S5 — the per-domain seal is forged from a downstream crate once M2 exposes it (S2), **and the ledger's own re-verification procedure is green on that tree** (S1). ⚠️ #41's stated reason is refuted by S4; the blanket impl works because the derive stops *naming* the seal. Not implemented in `crates/verum` — needs `Endpoint` (#60) |
| [0014](./0014-syntactically-present-replaces-observed.md) | `syntactically_present` replaces `observed`, and the lower bound is abandoned | accepted | `spikes/contract-from-tokens` D1/D2 — dead code is counted by **both** bounds (`if false` appears in the scan **and** still needs its `Has` bound), so the CI gate's difference is empty for it. Three names change; the `voided_by` join in `check_json.py` makes the rename mechanical |
| [0015](./0015-remedies-state-what-they-do-not-reach.md) | Every ledger cause states what its remedy does not reach; `#[domain]`'s forbidden-derive check is a lint | accepted | `spikes/domain-opacity-sqlx` — **P46** (the check works below the attribute), **P42** (it is blind above it, needled on the *absence* of verum's wording), **P43/P44** (placement is the mechanism; P44 mutation-verified), **P45** (path 28 run-verified). `spikes/ctx-lifetime-rpitit` **G1–G4** against the shipped `Repo`, G4 the control. `check_json.py`'s enumeration rules, five plants confirmed red. ⚠️ The never-blank column itself has **no guard** — it is a review rule |

> **Statuses live in two places** — an ADR's frontmatter and this row. #34 updated
> only the frontmatter and #39 repeated it, so before closing an ADR run
> `command grep -n '<number>' docs/adr/README.md` and check the row, the
> **By status** line, and any note below.

**By status** — proposed: **0004, 0006, 0007** · accepted: 0000–0003, 0005, 0008–0015 · superseded: none

> **Three `proposed` records, and the codebase relies on all three.** That is the
> state [ADR-0000](./0000-record-architecture-decisions.md) calls a defect, made
> visible rather than fixed. Each names the issue that settles it: #39 (with #40),
> path 9, path 14. **0004 is measured but not settled** — #15 showed the mechanism
> works and surfaced two preconditions nothing designs yet (the extension trait's
> shape, and `Repo` being unreachable outside `Ctx`). An earlier revision moved it
> to `accepted`; that verdict was withdrawn.
>
> **0005 is decided (#39); 0006 still has measurements without a decision.**
> That is the intended order — the spike measures, the issue decides.
>
> The coupling recorded here as "**#39 and #40 cannot be decided independently**" was
> **too broad, and #39 narrowed it**: what is coupled is which *field* carries the
> lifetime, not whether the parameter exists. 0005 settles the parameter and hands
> the field to #40 — along with the finding that the two candidates are *not*
> equivalent on the producer side, which is the input #40 needs. 0006's question
> remains larger than it was written: blocking the *constructor* does not block a
> *supplier*.

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
