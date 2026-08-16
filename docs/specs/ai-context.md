# AI Context / semantic code graph

Structured information that reduces what it costs an AI to explore the codebase.
A first-class artefact of the framework.

Related: [`unverified-boundaries.md`](./unverified-boundaries.md),
[`effect-system.md`](./effect-system.md),
[`effect-inference.md`](./effect-inference.md).

---

## The core idea

> Rather than making an AI read a large amount of source, make it read the
> semantic metadata first.

---

## Design principle: separate the writing side from the reading side

The contract in the source is short, and **the AI Context emits the complete,
expanded information.**

```text
source (contract)      → deltas and elisions. Token efficiency first
AI Context (JSON)      → the complete form. Zero ambiguity first
```

> **The cost of this separation**: it is a decision to accept that the source
> alone does not carry the complete meaning. In
> [`../concepts.md`](../concepts.md)'s trust ordering, the complete form intended
> for an AI sits on the **generated** side. Guaranteeing that output's freshness is
> therefore mandatory (below).

---

## Example output

```json
{
  "endpoint": "UpdateUser",
  "method": "PUT",
  "path": "/users/{id}",
  "domain": "User",

  "request":  "UpdateUserRequest",
  "response": "UserView",

  "reads": {
    "fields": ["User.id", "User.status", "User.name", "User.email"],
    "declared": ["User.id", "User.status"],
    "implied_by_mutates": ["User.name", "User.email"],
    "enforcement": { "level": "metadata_only", "scope": "none", "voided_by": "not_applicable" }
  },

  "mutates": {
    "unconditional": ["User.name"],
    "conditional": [
      { "condition": "EmailChanged", "fields": ["User.email"] }
    ],
    "effective": ["User.name", "User.email"],
    "enforcement": {
      "level": "upper_bound_checked",
      "scope": "handle_via_ctx",
      "voided_by": [
        "domain_repr", "domain_swap", "repository_impl", "service_body",
        "middleware", "constructor_body", "malformed_set",
        "upsert_granularity", "event_subscriber"
      ]
    },
    "observed": {
      "fields": ["User.name", "User.email"],
      "scope": "handle_only",
      "deferred": "unknown"
    }
  },

  "forbidden": {
    "fields": ["User.password_hash"],
    "enforcement": { "level": "intent_only", "scope": "declaration_only", "voided_by": "not_applicable" },
    "note": "Records intent. The macro checks only that no field appears in both `mutates` and `forbidden`. A field absent from `mutates` gets no capability, so calling its setter through `ctx` does not compile — but that is `mutates`' guarantee, with `mutates`' scope and `voided_by`, not this key's."
  },

  "creates": {
    "domains": ["AuditLog"],
    "enforcement": {
      "level": "upper_bound_checked",
      "scope": "handle_via_ctx",
      "voided_by": ["repository_impl", "service_body", "middleware",
                    "constructor_body", "malformed_set", "event_subscriber"]
    }
  },
  "deletes": {
    "domains": [],
    "enforcement": {
      "level": "upper_bound_checked",
      "scope": "handle_via_ctx",
      "voided_by": ["repository_impl", "service_body", "middleware",
                    "constructor_body", "malformed_set", "event_subscriber"]
    }
  },

  "effects": {
    "declared_delta": ["+CacheWrite"],
    "assumed_from_method": [
      "DatabaseRead", "DatabaseMutation", "CacheRead", "CacheWrite",
      "Logging", "Metrics", "Tracing"
    ],
    "derived_from": "method_default_table",
    "enforcement": { "level": "none", "scope": "none", "voided_by": "not_applicable" }
  },

  "unconditional": {
    "emits": ["UserUpdated"],
    "calls": [],
    "enforcement": {
      "level": "upper_bound_checked",
      "scope": "handle_via_ctx",
      "voided_by": ["repository_impl", "service_body", "middleware",
                    "constructor_body", "malformed_set", "event_subscriber"]
    }
  },

  "conditional": [
    {
      "condition": "EmailChanged",
      "condition_defined_at": "src/conditions/user.rs:12",
      "condition_verified": false,
      "mutates": ["User.email"],
      "emits":   ["EmailVerificationRequested"],
      "calls":   ["EmailService"],
      "enforcement": {
        "level": "upper_bound_checked",
        "scope": "handle_via_ctx",
        "voided_by": ["repository_impl", "service_body", "middleware",
                    "constructor_body", "malformed_set", "event_subscriber"]
      }
    }
  ],

  "dependencies": ["UserRepository", "AuditLogRepository", "EventBus", "EmailService"],

  "atomicity": "none",
  "model_fit": "unknown",

  "escape_hatches": "unknown",

  "unverified_boundaries": {
    "completeness": "best_effort",
    "entries": [
      { "kind": "condition_body", "detail": "EmailChanged::holds cannot be verified in types",
        "location": "src/conditions/user.rs:12", "permanent": true },
      { "kind": "row_scope", "detail": "row-level permissions are outside the type check; authorisation is separate",
        "permanent": true },
      { "kind": "middleware", "detail": "the effects of the applied middleware are undeclared",
        "permanent": false },
      { "kind": "event_subscriber", "detail": "effects on the subscriber side of UserUpdated are unchecked",
        "permanent": false },
      { "kind": "constructor_body", "detail": "AuditLog::user_updated is called from handle; its purity is convention, not a check (path 18)",
        "permanent": false },
      { "kind": "upsert_granularity", "detail": "creates plus deletes on one domain changes field values with no Mutate capability (path 19)",
        "permanent": false },
      { "kind": "uncapped_read", "detail": "a Domain's Debug and free functions read fields outside the endpoint's reads; no getter shape reaches them, and a Projection narrows only its own Debug (path 23)",
        "permanent": false },
      { "kind": "service_body", "detail": "the observed_effects scan is neither complete nor sound: it cannot leave its own item, it matches receivers by spelling, and it runs before cfg-stripping so it reports effects from code that is never compiled (path 22)",
        "permanent": false },
      { "kind": "domain_repr", "detail": "a domain's Repr is reachable from anywhere in the same crate (path 21)",
        "location": "src/domain/user.rs", "permanent": false },
      { "kind": "malformed_set", "detail": "a malformed effect set can be passed through the capability check (path 14f)",
        "permanent": false },
      { "kind": "domain_swap", "detail": "*user = other_user cannot be closed (path 2)",
        "permanent": true },
      { "kind": "repository_impl", "detail": "SQL inside a repository implementation is unchecked",
        "location": "src/repositories/user.rs", "permanent": false }
    ]
  }
}
```

---

## Seven things the output must always carry

Verum-specific requirements, absent from ordinary documentation generation.

### 1. `enforcement` — the level, the scope, and what voids it

Every key that claims a guarantee carries all three. A level alone says how
strong the check is and says nothing about **where it stops**, and it is at the
edges that a reader forms false beliefs.

```json
"enforcement": {
  "level": "upper_bound_checked",
  "scope": "handle_via_ctx",
  "voided_by": ["domain_repr", "repository_impl", "..."]
}
```

**`level`** — how strong the check is.

| Value | Meaning |
|---|---|
| `upper_bound_checked` | Type-checked, but only the **upper bound** of implementation ⊆ contract. An effect declared but unused is not detected |
| `intent_only` | A record of intent. Redundant as far as the type check goes (`forbidden` — [`mutation-contract.md`](./mutation-contract.md)) |
| `metadata_only` | Declaration only. No type check. There is no guarantee the implementation follows it |
| `none` | An axis with no type check at all (infrastructure effects and the like) |

> **The value `type_checked` is not used.** It reads as "verified in both
> directions", whereas Verum's check is an upper bound only.
> `mutates = [name, email]` does not mean "it changes name and email" but "**it
> changes nothing but name and email**". That distinction decisively affects an
> AI's reasoning.

**`scope`** — how far the check reached. A closed vocabulary, for the same reason
the infrastructure-effect vocabulary is closed
([`effect-system.md`](./effect-system.md)): two spellings of one idea leave an AI
unable to choose.

| Value | Meaning |
|---|---|
| `handle_via_ctx` | Only calls routed through `ctx` inside `handle` were checked. Repository implementations, service bodies, middleware and free functions were not read |
| `declaration_only` | The declaration's internal consistency was checked. No implementation was read |
| `none` | Nothing was checked |

**`voided_by`** — the `kind` names of the boundaries that make this key's claim
false. **Every name here appears in this same output's
`unverified_boundaries.entries`**, so the reader can join the two without leaving
the artefact. That join is the whole mechanism: recording a bypass somewhere in
the document does not help a consumer who reads `mutates.enforcement` and stops.

When `level` is `metadata_only` or `none` there is no guarantee to void, so
`voided_by` is the string `"not_applicable"` — **never `[]`**, which reads as
"nothing voids this" (the same reasoning as `escape_hatches`, §7).

The assignment per key, derived from what each key actually claims:

| Key | `level` | `scope` | `voided_by` |
|---|---|---|---|
| `reads` | `metadata_only` | `none` | `not_applicable` |
| `mutates` | `upper_bound_checked` | `handle_via_ctx` | 9 kinds |
| `creates` / `deletes` | `upper_bound_checked` | `handle_via_ctx` | 6 kinds |
| `unconditional` / each `conditional` entry | `upper_bound_checked` | `handle_via_ctx` | 6 kinds |
| `forbidden` | `intent_only` | `declaration_only` | `not_applicable` |
| `effects` | `none` | `none` | `not_applicable` |

`emits` and `calls` are easy to overlook here because they sit inside
`unconditional` and `conditional` rather than at the top level — but
`"calls": []` is as much a claim of absence as `deletes.domains: []`, and it goes
through `ctx` in exactly the same way. It carries the same enforcement block.

Two exclusions from `mutates` are deliberate. **`row_scope` voids which *row*, not
which *field***, and this key's claim is about fields — the wrong row still gets
only declared fields changed. **`condition_body` does not widen the set** either:
`User.email` is in `effective` whether or not the condition is honest; the
dishonest-condition case is what `condition_verified: false` answers (§4).
`creates` and `deletes` additionally drop `domain_repr` (forging a value does not
persist it) and `upsert_granularity` (it bypasses field granularity, not the
domain declaration).

Hide the difference in enforcement level and an AI trusts parts of the contract
without knowing they are no better than a comment. Hide the scope and it trusts
the enforced parts **further than they reach**, which is the harder error to
notice. Rationale in
[ADR-0008](../adr/0008-guarantees-carry-scope-and-voiding-paths.md).

#### `observed` — the lower bound (the Q-A decision, 2026-08-15)

What `enforcement` answers is only "**nothing else happens**". "**This
happens**" is a separate field, **generated** by scanning `handle`'s tokens.

```json
"observed": { "fields": [...], "scope": "handle_only", "deferred": "unknown" }
```

| Key | Meaning |
|---|---|
| `fields` | The effects the scan **found written** inside `handle`. **Generated, never hand-written** |
| `scope` | How far the scan reached. `"handle_only"` in the First PoC — ⚠️ **the value overstates itself**: T-M1-07 measured that the scan does not cover all of `handle`. It was added so an AI would not misread the lower bound as covering every path, and it now produces a smaller version of that same misreading. Replacement tracked on path 22 |
| `deferred` | Items escaped via `@service`. A service body is not scanned, so anything appearing here also raises a `service_body` entry in `unverified_boundaries`. **That `kind` now under-describes its own boundary** — T-M1-07 showed most of the misses are inside `handle`, not in a service. Renaming it is tracked on path 22 |

**`observed.fields` is syntactic presence, not execution.** The scan reads
tokens: a `set_email` written inside a `when` block, behind an `if`, or on a path
that never runs appears here exactly as an unconditional one does. So
`observed.fields == effective` does **not** mean every listed field changes on
every request — for that, read `conditional` and `condition_verified` (§3).
Stated because the pair `observed.fields` + the reading rule below is what
produced the belief "name and email are changed, no more and no less", and only
the "no more" half of that is something this output can support.

**`deferred` emits `"unknown"`, never `[]`.** The `@service` marker is
self-reported in exactly the way `escape_hatches` is (§7): forget it and a
service still performs the effect, while the output reads `[]` and no
`service_body` boundary is raised. An empty array here would claim "nothing was
deferred", which is precisely the claim nobody can make.

> The upgrade path is for the token scan to emit **every call it could not
> follow**, whether or not it is marked — at which point `[]` becomes honest and
> `@service` supplies only the reason. That depends on #37 (does token scanning
> recover the contract at all) and on #42, which disputes whether `observed` is a
> lower bound in the first place. **`observed` is otherwise unchanged here**, so
> that this file does not prejudge either.

**How to read it** — ⚠️ **corrected by T-M1-07 (#37); the earlier rule was
false.** It said: if `enforcement: upper_bound_checked` and
`observed.fields == effective` and `deferred` is empty, then **within that
`scope` the set is exact**. It is not. The scan misses effects that are inside
`scope: "handle_only"` — a receiver bound to a local, a renamed `ctx` parameter,
a UFCS call — and it reports effects from code that is never compiled. So:

* `observed.fields == effective` means the two agree, **not** that either is
  complete.
* **`observed` is not a lower bound.** It is syntactic presence within one item,
  matched by spelling.
* The only exactness claim that survives is the upper bound, from
  `enforcement`, with the `scope` and `voided_by` that key carries.

Path 22 records the six measured constructs.

`declared \ observed ≠ ∅` — over-declaration — is failed by CI, so **an
over-declaration does not normally survive into this output.** If one has, look at
`deferred`. **No `enforcement` value meaning "verified in both directions" is
created** — the same reason `type_checked` is banned: folding different layers
into one word invites misreading. Detail in
[`effect-inference.md`](./effect-inference.md) §Decision (Q-A).

### 2. `effective` vs `assumed_from_method` — an expansion is not an observation

`mutates.effective` is a genuine expansion of what the user declared:
`unconditional ∪ conditional`, every element of it written by hand in the
contract.

`effects` has no such declaration to expand. Its list is produced from the HTTP
verb by a table [`effect-system.md`](./effect-system.md) itself calls **"a
documentation table with zero type checking"** — nothing read the implementation.
It is therefore emitted as **`assumed_from_method`**, with `derived_from` naming
the table:

```json
"effects": {
  "declared_delta": ["+CacheWrite"],
  "assumed_from_method": ["DatabaseRead", "..."],
  "derived_from": "method_default_table",
  "enforcement": { "level": "none", "scope": "none", "voided_by": "not_applicable" }
}
```

**The key was called `effective` and it was the name that misled**, not a missing
caveat: `enforcement: "none"` already sat beside it and readers still concluded
"this endpoint causes DatabaseRead and CacheWrite". A value derived from a lookup
table must not wear the name that means "the fully expanded declaration"
everywhere else in the same document.

The original benefit is kept — an AI can still decide without knowing the
framework's default specification — because the list is still emitted. What
changes is that it no longer claims to be an observation.

### 3. The distinction between `unconditional` and `conditional`

Do not mix "always happens" with "happens depending on a condition".

**`condition_verified: false` may not be omitted.** The body of a condition
cannot be verified in types, so leaving it out makes **the metadata actively
lie** — it says `conditional`, while `holds` may just return `true`. The ledger
`kind` for this is `condition_body`; it is why an honest `mutates.effective` can
still overstate *when* a field changes, and why that is answered here rather than
in `mutates.enforcement.voided_by` (§1).

### 4. `unverified_boundaries` — where the type check does not reach, and how completely we know

```json
"unverified_boundaries": { "completeness": "best_effort", "entries": [ ... ] }
```

The full ledger of routes is in
[`unverified-boundaries.md`](./unverified-boundaries.md). `permanent: true` marks
what cannot be closed in principle.

**`completeness` is `best_effort`, never `exhaustive`.** A bare list claims to be
every path the type check does not reach, and that is not a claim anyone can
check: the ledger itself records a path marked closed while it was open **three
times** (#6 / #8 / #9), and one review added four more entries. This is the
`escape_hatches` treatment (§7) applied to the list as a whole rather than to a
single value.

**Each entry's `kind` is what `enforcement.voided_by` names** (§1). The set of
kinds emitted here and the set named across every `voided_by` must agree; that
agreement is checked mechanically by `spikes/doc-code-blocks/check_json.py`, not
promised.

**This output mechanism is implemented from the First PoC.** Added later, it would
mean every AI Context up to that point had been lying.

### 5. `model_fit` — whether the endpoint's shape is inside the model

```json
"model_fit": "unknown"
```

| Value | Meaning |
|---|---|
| `single_instance` | The endpoint operates on one domain instance, which is what the model covers |
| `outside_model` | A list, search, aggregation or JOIN endpoint. The contract vocabulary does not describe its shape |
| `unknown` | Not determined |

`Read<Domain, Field>` assumes a single instance, and there is **no vocabulary at
all** for lists, search, aggregation or JOIN
([`research-questions.md`](./research-questions.md) §Listing, search,
aggregation, JOIN). Without this key, `GET /users?status=active&page=2` emits
JSON indistinguishable from `GET /users/{id}` — a `reads` list, an enforcement
level, a boundary list — and nothing says the endpoint's shape falls outside what
any of it describes. By endpoint count that is likely the largest honesty gap in
the artefact.

**`"unknown"` is what is emitted today**, because nothing determines the value:
the model has no way to express a list endpoint, so there is no declaration to
read it from. It is promoted to a real value once that vocabulary exists.

### 6. `atomicity` and `dependencies` — stated, not checked

Neither is a guarantee, and both sit in the same shape as keys that are, so both
are defined here rather than left to be inferred.

| Key | What it is |
|---|---|
| `atomicity` | `"none"` today. A contract is an upper bound, so "only a subset of the declared effects happened" is not expressible; the transaction boundary is undesigned ([`persistence.md`](./persistence.md)) |
| `dependencies` | Derived from the declared domains and services. A list of what the endpoint names — **not** a check that it uses them, nor that it uses nothing else |

### 7. `escape_hatches`

```json
"escape_hatches": "unknown"
```

Recording an escape hatch is **self-reported** today — forget the attribute and
nothing is recorded. So an empty array `[]` must not be emitted. `[]` reads as
"no escapes", whereas the truth is "there may be an unreported escape".

Once the low-level API requires a ZST proof produced by an attribute macro as an
argument, a missing record becomes structurally impossible and `[]` can be
emitted.

### Removed: `scope_of_readonly_guarantee`

It was the first key to name its own scope, and this file's schema is that idea
generalised — which made it redundant. A GET's read-only guarantee is now stated
exactly by `mutates`, `creates` and `deletes` all being empty at
`level: upper_bound_checked`, `scope: handle_via_ctx`, with `middleware` listed
under `voided_by`. **A value derivable from its neighbours is a value that can
disagree with them**, and this one carried the whole middleware caveat alone.

It also overstated. A GET may cause Logging, Metrics, Tracing, CacheRead and
CacheWrite ([`effect-system.md`](./effect-system.md)), so nothing about it was
read-*only*; the accurate claim is about state effects, which is what the three
keys above cover. And it was emitted on a `PUT` in this file's own canonical
example, announcing the scope of a guarantee that endpoint did not have.

Rationale in
[ADR-0008](../adr/0008-guarantees-carry-scope-and-voiding-paths.md).

---

## Guaranteeing the generated output's freshness

With both the source and the JSON present, **if an AI cannot tell which to
believe, this design merely reproduces "do not trust comments" in JSON.**

| Means | Contents |
|---|---|
| Keep it out of git | Generate it as part of the build and do not commit it, so a stale file never exists |
| A zero-diff check in CI | Regenerate and fail if a diff appears |
| Include a timestamp | Embed the generation time and a hash of the source in the JSON |

---

## When an AI reads it — the operating procedure has to be defined

**Designing a schema is pointless if an AI never reads it.** A coding agent reads
the source directly unless told otherwise.

So the following is needed, and is currently undefined:

- State the step "read `cargo verum contract` before touching an endpoint" in the
  equivalent of a `CLAUDE.md`
- Fix on a single output command
- Provide a separate minimal reference for AI use — the framework's conventions
  compressed into roughly 100 lines

**Without that procedure, the AI Context becomes an artefact that is built and
never read.** Recorded in [`research-questions.md`](./research-questions.md).

---

## Output formats

| Format | Use | Priority |
|---|---|---|
| JSON | AI Context / CI verification | First PoC |
| Markdown | Human-facing documentation | Full PoC |
| OpenAPI | Interoperating with existing toolchains | Full PoC |
| MCP | Serving AI agents dynamically | After the Full PoC |

---

## Implementation approach

A derive macro plus `inventory` (or `linkme`) collects each endpoint's contract
at compile time.

```text
#[derive(Endpoint)] → registers the contract in inventory
        ↓
cargo verum contract --format json
```

The same mechanism is reusable for generating the compile-time route table
([`runtime-stack.md`](./runtime-stack.md)).

---

## Open problems

- **Managing the context's size** — roughly 400–600 tokens per endpoint. At 200
  endpoints that is around 100k tokens, which collides with the claim of "tens to
  hundreds of tokens instead of thousands of lines". Splitting, summarising, or
  fetching only the endpoints needed is required
- Whether relationships between endpoints (an event's emitter and its subscribers)
  are expressed as a graph
- Versioning the schema

See [`research-questions.md`](./research-questions.md).
