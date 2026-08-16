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
    "enforcement": "metadata_only"
  },

  "mutates": {
    "unconditional": ["User.name"],
    "conditional": [
      { "condition": "EmailChanged", "fields": ["User.email"] }
    ],
    "effective": ["User.name", "User.email"],
    "enforcement": "upper_bound_checked",
    "observed": {
      "fields": ["User.name", "User.email"],
      "scope": "handle_only",
      "deferred": []
    }
  },

  "forbidden": {
    "fields": ["User.password_hash"],
    "enforcement": "intent_only",
    "note": "Not type-enforced. Fields absent from `mutates` are already uncallable; this records intent."
  },

  "creates": { "domains": ["AuditLog"], "enforcement": "upper_bound_checked" },
  "deletes": { "domains": [], "enforcement": "upper_bound_checked" },

  "effects": {
    "declared_delta": ["+CacheWrite"],
    "effective": [
      "DatabaseRead", "DatabaseMutation", "CacheRead", "CacheWrite",
      "Logging", "Metrics", "Tracing"
    ],
    "enforcement": "none"
  },

  "unconditional": {
    "emits": ["UserUpdated"],
    "calls": []
  },

  "conditional": [
    {
      "condition": "EmailChanged",
      "condition_defined_at": "src/conditions/user.rs:12",
      "condition_verified": false,
      "mutates": ["User.email"],
      "emits":   ["EmailVerificationRequested"],
      "calls":   ["EmailService"]
    }
  ],

  "dependencies": ["UserRepository", "AuditLogRepository", "EventBus", "EmailService"],

  "scope_of_readonly_guarantee": "handler_only",
  "atomicity": "none",

  "escape_hatches": "unknown",

  "unverified_boundaries": [
    { "kind": "condition_body", "detail": "EmailChanged::holds cannot be verified in types",
      "location": "src/conditions/user.rs:12", "permanent": true },
    { "kind": "row_scope", "detail": "row-level permissions are outside the type check; authorisation is separate",
      "permanent": true },
    { "kind": "middleware", "detail": "the effects of the applied middleware are undeclared",
      "permanent": false },
    { "kind": "event_subscriber", "detail": "effects on the subscriber side of UserUpdated are unchecked",
      "permanent": false },
    { "kind": "service_body", "detail": "the observed_effects scan covers only the inside of handle; effects in a service body do not appear in the lower bound (path 22)",
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
```

---

## Six things the output must always carry

Verum-specific requirements, absent from ordinary documentation generation.

### 1. `enforcement` — the enforcement level

States **how far each contract item is guaranteed by types.**

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

Hide the difference in enforcement level and an AI trusts parts of the contract
without knowing they are no better than a comment.

#### `observed` — the lower bound (the Q-A decision, 2026-08-15)

What `enforcement` answers is only "**nothing else happens**". "**This
happens**" is a separate field, **generated** by scanning `handle`'s tokens.

```json
"observed": { "fields": [...], "scope": "handle_only", "deferred": [] }
```

| Key | Meaning |
|---|---|
| `fields` | The effects that actually occur inside `handle`. **Generated, never hand-written** |
| `scope` | How far the scan reached. `"handle_only"` in the First PoC. **Without it, an AI misreads the lower bound as covering every path** |
| `deferred` | Items escaped via `@service`. A service body is not scanned, so anything appearing here also raises a `service_body` entry in `unverified_boundaries` |

**How to read it**: if `enforcement: upper_bound_checked` and
`observed.fields == effective` and `deferred` is empty, then within that `scope`
**the set is exact.** If any one of the three is missing, it is not.

`declared \ observed ≠ ∅` — over-declaration — is failed by CI, so **an
over-declaration does not normally survive into this output.** If one has, look at
`deferred`. **No `enforcement` value meaning "verified in both directions" is
created** — the same reason `type_checked` is banned: folding different layers
into one word invites misreading. Detail in
[`effect-inference.md`](./effect-inference.md) §Decision (Q-A).

### 2. `effective` — the complete effect set after expansion

The source carries only the delta `effects = [+CacheWrite]`, while the output
gives the complete form with the per-method defaults expanded. An AI can then
decide without knowing the framework's default specification.

### 3. The distinction between `unconditional` and `conditional`

Do not mix "always happens" with "happens depending on a condition".

**`condition_verified: false` may not be omitted.** The body of a condition
cannot be verified in types, so leaving it out makes **the metadata actively
lie** — it says `conditional`, while `holds` may just return `true`.

### 4. `unverified_boundaries` — where the type check does not reach

The full ledger of routes is in
[`unverified-boundaries.md`](./unverified-boundaries.md). `permanent: true` marks
what cannot be closed in principle.

**This output mechanism is implemented from the First PoC.** Added later, it would
mean every AI Context up to that point had been lying.

### 5. `scope_of_readonly_guarantee`

| Value | Meaning |
|---|---|
| `handler_only` | Read-only inside the handler. Middleware may still mutate |
| `request` | Read-only for the whole request (after middleware contracts arrive) |

"A GET is read-only" must not be claimed unconditionally.

### 6. `escape_hatches`

```json
"escape_hatches": "unknown"
```

Recording an escape hatch is **self-reported** today — forget the attribute and
nothing is recorded. So an empty array `[]` must not be emitted. `[]` reads as
"no escapes", whereas the truth is "there may be an unreported escape".

Once the low-level API requires a ZST proof produced by an attribute macro as an
argument, a missing record becomes structurally impossible and `[]` can be
emitted.

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
