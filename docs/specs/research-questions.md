# Research questions

The unresolved design problems. Everything Verum competes on lives here.

Related: [`unverified-boundaries.md`](./unverified-boundaries.md).

---

## The three to settle first

### ⚠️ Q-A. Whether to adopt "generate the contract from the implementation" — **decided 2026-08-15, REOPENED 2026-08-16**

**Adopt it — but not as a replacement for type enforcement. Keep both and make
the difference the detector.**

There are two goals and they require separate mechanisms. "Leave no bypass" =
upper bound = type enforcement (already in place); "leave no lie" = lower bound =
generation (types cannot produce it in principle). **The difference between the
two solves exactly the open problem in §Detecting over-declaration below.**

| Decided | Contents |
|---|---|
| Approach | Keep type enforcement as it is, plus generate `observed_effects` by scanning `handle`'s tokens |
| Scope | The First PoC covers **one item — and not all of it** (T-M1-07: the scan matches receivers by spelling and cannot follow a call into another item), stated in the AI Context as `scope: "handle_only"` |
| Over-declaration | **CI fails.** None of the three defence layers — a fourth mechanism at build time. The `@service` escape records its own use |
| Verifying the premise | **Ran 2026-08-16 as T-M1-07 (#37): the premise does not hold.** Three of five contract keys recover; the scan is neither complete nor sound. Originally added to Phase 1 as spike **T-M1-07** (whether token scanning works is unverified) |

The full account and the four rejected options are in
[`effect-inference.md`](./effect-inference.md) §Decision (Q-A).

### Q-B. Is the token budget really positive?

`../concepts.md`'s "tens to hundreds of tokens instead of thousands of lines"
does not distinguish between situations.

| Task | Budget |
|---|---|
| Surveying many endpoints | Positive |
| **Editing one endpoint** | **Negative** (both the contract and the implementation have to be read, plus knowledge of the conventions) |

Real AI coding is mostly the latter. On top of that, the AI Context is 400–600
tokens per endpoint, around 100k at 200 endpoints. Verum has roughly 40 concepts
(Axum 8–10, Rails 15–20), and because they are absent from the training data they
have to be loaded into context every session.

**The break-even point has to be produced as a number.** The judgement includes
the option of repointing the objective from "fewer tokens" to "make the compiler
the AI's feedback loop".

### ~~Q-C. When to run the premise check~~ → **done (2026-08-14)**

The experiment's design, results, break-even point (about 5 endpoints) and kill
criteria are in [`evaluation.md`](./evaluation.md) §The experiment already run.
One outcome was removing `operation`, recorded in §`operation` below.

What follows is the text as written at the time of the decision.

`evaluation.md`'s nine metrics have no measurement method, no judge, no trial
count, no pass threshold and **no kill criteria.** And they were scheduled for
Phase 2 — after every resource had gone into the type design.

There are three measured criteria for dropping Axum, but **none for stopping or
repointing the project itself** — an inversion.

There is an experiment that needs no code at all: give an AI the same problem
under three conditions — plain Axum, Axum with structured annotations, and
hand-written Verum pseudo-code plus a cheatsheet — and measure tokens, iterations
and violation rate.

---

## Questions whose direction is settled

| # | Question | Decision | Reference |
|---|---|---|---|
| 1 | How finely can effects be expressed in types? | Three families: state / external / infrastructure. Per-category associated types. The vocabulary is closed | [`effect-system.md`](./effect-system.md) |
| 2 | How is field-level mutation expressed in types? | **Make the domain opaque**, with a ZST marker per field and capability-requiring setters | [`mutation-contract.md`](./mutation-contract.md) |
| 3 | How are conditional effects expressed? | Not in types. Capabilities are issued in a `ctx.when::<C>` scope (async closures required). **A conditional mutation / emit / call is declared inside `when`, and duplicating it at the top level is forbidden** (the hole the Q-C experiment found, now specified) | [`conditional-effects.md`](./conditional-effects.md) |
| 5 | How is the capability system designed? | `Ctx<'req, E>` is parameterised by the contract, and checked in an **extension trait**'s where clause | [`capability-system.md`](./capability-system.md) |
| 6 | How is a GET's read-only guarantee proved? | `Endpoint<Mutates=(), Creates=(), Deletes=()>` plus **a compile-time assertion from the derive** (a blanket impl cannot do it) | [`rust-type-model.md`](./rust-type-model.md) |
| 8 | How is the architecture contract enforced? | Put `Self::Owner: Includes<User>` in the method's where clause (`Includes`'s subject is the endpoint type — [ADR-0001](../adr/0001-includes-is-implemented-on-the-endpoint.md)) | [`architecture-contract.md`](./architecture-contract.md) |
| Q-A | Whether to generate the contract from the implementation | **Keep both and make the difference the detector.** Type enforcement = upper bound (bypasses); generation = lower bound (lies). Over-declaration fails CI. Scope is `handle` only in the First PoC. ⚠️ **REOPENED 2026-08-16** — T-M1-07 (#37) measured the premise and it does not hold: three of five keys recover, and the scan is neither complete nor sound. | [`effect-inference.md`](./effect-inference.md) |
| 10 | Is a proc macro alone enough? | Three defence layers (macro / equality bound / trait bound) **plus a build-time token scan** (added by the Q-A decision; a fourth mechanism producing the difference between declaration and implementation, outside the proc macro). A custom linter is needed only for escape hatches and raw SQL | [`diagnostics.md`](./diagnostics.md) |

### Technical constraints settled by compiling

| Point | Conclusion |
|---|---|
| `Has`'s recursive impls | The naive form violates coherence (E0119). **The index-parameter version is mandatory** |
| Representing an effect set | A flat tuple cannot decide membership. **Cons lists throughout** |
| `Endpoint<METHOD = Method::GET>` | Associated-const equality bounds are unstable, and the blanket impl's logic does not work. **Method becomes a type-level marker** |
| `impl<E> Ctx<E> { fn users() }` | E0116. **An extension trait is mandatory** |
| `when`'s borrows | Lending `&user` while using `async move` is a borrow error. **Async closures (edition 2024 / 1.85+) are mandatory** |
| A `Handler` using AFIT | dyn incompatible, and the future is not Send. **RPITIT + Send + an erasure layer** |
| Type-level operations | `Has` / `Append` / `Lookup` are safe. `Subset` / `Filter` / negative reasoning are not |
| `on_unimplemented` | The message is controllable, but the help/note chain remains. **`do_not_recommend` is mandatory** |
| A `note` pointing at the contract declaration | **Not emittable** through a trait bound. Only through an equality bound |

---

## What the First PoC must verify

| Item | What is unknown | Impact if it fails |
|---|---|---|
| ~~**Domain opacity × sqlx**~~ | **Settled (T-M1-01 / #13, compile-verified)**: the sqlx interoperation holds. "The trust boundary is the repository implementation" **does not** (`Repr` is reachable from anywhere in the same crate — ledger path 21). And the derive the specification names cannot generate the required shape. Detail in [`persistence.md`](./persistence.md) §Verdict; reproduction in `spikes/domain-opacity-sqlx/` | — |
| ~~**`Ctx<'req, E>` × async**~~ | ~~Whether the combination with RPITIT and async closures holds~~ | **Settled (T-M1-02 / #14, 21 probes).** All four criteria hold. What the probes added beyond confirmation: `ctx.spawn` as specified does not compile (F1 — #40), capability handles escape `'req` (E1 — #39), and ledger path 8's recorded remedy is not its mechanism — the specified signature is closed by the higher-ranked `Ctx`, while a **named `'req` leaks and is reachable from an ordinary handler** (RK-017) |
| **Whether domain getters alone suffice to enforce `reads`** | The projection type may be unnecessary | If they suffice, the whole complexity of projections is removed |
| **`trybuild`'s stability** | Whether error text containing cons lists and `There<There<..>>` shifts between rustc versions | If it shifts, a normalisation mechanism is needed |
| **Compile time** | The cost of resolving `Has` across endpoint count × effect count | If it degrades, shrink the scope of type enforcement |

---

## Open problems in expressing a contract

### Listing, search, aggregation, JOIN (highest priority)

**A consequence of `Read<Domain, Field>` assuming a single instance.** The shape
that accounts for the most screens in a real web application cannot be written.

- Listing: an API returning `Vec<Projection<..>>`, and expressing count, ordering
  and dynamic filters
- Pagination: the filter is decided at run time, so it does not fit a type
  parameter
- Aggregation (COUNT / SUM / GROUP BY): not the value of a particular field, and
  the result belongs to no domain instance
- JOIN: a composite projection (`Projection<(User, Order), (..)>`) is undefined
- **N+1 / eager loading**: collides structurally with per-field methods (Rule 1).
  An exception on the read side is needed

This is not a missing feature but **a crack in the model's premise** — the result
of building the whole model from single-fetch-by-ID examples, `GetUser` and
`UpdateUser`.

### Validation

There is no mechanism for declaring a request's constraints (required, range,
format). `reads` and `mutates` are permissions over a domain's fields; a request's
fields are out of scope.

An area that in practice is a fertile source of bugs and security problems falls
outside what "the compiler rejects contract violations" claims to cover.

### Errors

- Whether to declare them as `fails = [NotFound, Conflict]`
- Where the mapping between HTTP status and domain error is defined
- Whether an error is treated as a kind of effect
- Required for OpenAPI generation

### Transactions

- Is endpoint = one transaction the standard?
- **Can firing an external effect inside a transaction be forbidden in types?**
  (the `ctx.after_commit` scope proposed in
  [`handler-rules.md`](./handler-rules.md) Rule 4)
- **The semantics of partial failure** — a contract is an upper bound, so "only a
  subset of the declared effects happened" is not expressible. Emitting
  `atomicity: "none"` is the interim response
- Savepoints and nested transactions

### Soft delete (high priority)

`mutates = [User::deleted_at]` and `mutates = [User::name]` are **syntactically
indistinguishable.** To an AI reading the contract, "change the name", "soft
delete" and "restore" all look the same.

**"Semantics over syntax" fails on the most common CRUD pattern.** Whether an
additional tag such as `SoftDelete<Domain>` / `Restore<Domain>` is needed.

### Optimistic and pessimistic locking

- Per-field setters cannot express `WHERE id=? AND version=?` compare-and-swap as
  an atomic operation
- The mutation contract's premise — that each field succeeds or fails
  independently — breaks down
- There is no `Lock<Domain>` effect corresponding to `SELECT ... FOR UPDATE`

### Bulk operations

How a 100-row batch update is written with per-field setters. Whether a
distinction of cardinality such as `One<User>` / `Many<User>` is needed.

### Static capabilities and dynamic authorisation

```text
static (compile time) — "what can this endpoint do"
dynamic (run time)    — "what is this caller allowed to do"
```

The current design covers only the former. **And the wording of
`../concepts.md`'s principle "capability over permission checks" invites the
misreading that authorisation can be replaced by capabilities.**

- Whether `authz` becomes a required contract item (`authz = [Owner]` /
  `authz = [Public]`)
- Whether an `authorization` field is added to the AI Context, with empty
  disallowed
- Stating that row-level permissions (IDOR) are outside the type check

### Multi-domain endpoints

- Should a domain appearing in `creates` or `emits` become accessible
  automatically?
- Should an endpoint be allowed to update two business-independent domains at
  once?
- Transactions across an aggregate boundary

### State-transition contracts

Whether `status: active → suspended` is expressed in the contract. Whether
typestate can do it. Whether exhaustiveness of transitions can be checked.

### Composing conditions

The capability-composition rules for `when(A and B)` / `when(A or B)` /
`when(not A)`. `not` runs into the negative-reasoning problem.

### Making conditions asynchronous

`Condition::holds` is defined as a synchronous pure function. Feature flags, A-B
tests and clock-dependent conditions need external I/O and cannot be expressed.
Extending to `async fn holds(ctx: &Ctx<..>, ..)` requires reconsidering its
consistency with the effect system.

### Jobs and background work

The `Endpoint` framing (method + path + request/response) does not apply to
processing with no HTTP request. A first-class contract unit such as
`#[job(schedule = "...")]` is needed.

### `operation` — resolved by deleting it (an outcome of the Q-C experiment)

**Deleted.** Detail in [`semantic-endpoint.md`](./semantic-endpoint.md).

The experiment's subject reported:

> The set of values `operation` can take is unknown (only `Read` / `Update` /
> `Suspend` / `Delete` observed). I hesitated over whether to create a dedicated
> value like `UpdateEmail` for the new endpoint, but **to avoid the risk of
> inventing an enum variant that does not exist** I reused the existing `Update`.

It was a field the AI hesitated over every time while guaranteeing nothing, and
the information is derivable from `Method` plus `Domain` plus `mutates` plus **the
endpoint's type name**, so it was deleted. The business operation name is carried
by the type name (`SuspendUser`).

The related observation "the list of supported HTTP methods is unwritten" was
resolved at the same time (Get / Head / Post / Put / Patch / Delete are now
stated).

> **The Q-C experiment found three gaps in the specification by collecting "where
> the AI hesitated"** (where a conditional mutation is declared, `forbidden`'s
> semantics, `operation`). Collect it in every future experiment.

### Middleware contracts

Middleware effects appear neither in the contract nor in the AI Context. If auth
middleware updates `last_login_at`, "a GET is read-only" becomes true only at
handler scope.

A mechanism is needed for the router to compose "the endpoint's declaration plus
the declarations of every middleware applied".

### Contracts on the subscriber side of an event

Declaring `emits` costs almost nothing, but a subscriber can cause arbitrary
effects. **Emit has become a general-purpose gateway to any effect.** Emitting the
transitive closure in the AI Context requires a contract on the subscriber side
too, which may live in a different crate.

---

## Open problems in the implementation

### Generating the repository trait definition (high priority)

While `set_<field>` is hand-written per field in both the trait and the impl:

1. Boilerplate grows with every new domain, and the token-efficiency claim
   collapses on the *writing* side
2. **Writing `user::Name` by mistake in `set_email`'s where clause is not
   detected** — the claim "rustc does the matching for us" depends on the
   hand-written boilerplate being correct

Generating the trait definition should be moved ahead of generating the impl.

### ⚠️ Detecting over-declaration — **was marked solved by the Q-A decision; reopened by T-M1-07**

A declared but unused capability is not an error (a consequence of the contract
being an upper bound). **`declared_ceiling \ observed_effects` is the detector** —
Q-A (2026-08-15) positioned taking that difference as the main purpose rather than
a by-product. CI fails on it.
[`effect-inference.md`](./effect-inference.md) §Decision (Q-A).

### Detecting database mutations

SQL inside a repository implementation is outside the trust boundary. Either move
the boundary by generating the implementation, or statically check sqlx `query!`'s
columns (which needs SQL parsing and is powerless against dynamic SQL).

### The quality of error messages

- Whether the derive can generate type aliases to shorten type names in errors
- The error when firing outside a `when` scope (nested projections get exposed)
- Dealing with "no method named `users`" from forgetting to `use` an extension
  trait (auto-generating the prelude / `pub use`)

### Enforcing the handler rules

- Rule 1: a lint for when a user adds a blanket method of their own
- Rule 2: the purity of free associated functions (`AuditLog::user_updated` and
  the like). Whether `#[derive(Event)]` / `#[derive(View)]` can generate the
  constructors and remove the room for hand-writing

### Enforcing infrastructure effects

Currently `enforcement: "none"`. Either enforce it in `ctx.cache()`'s where
clause, or **drop the axis entirely.** It has the lowest enforcement per concept of
any contract item.

---

## Open problems in practical AI coding

### Contract-relaxation bias (high priority)

Faced with a compile error, an AI **widens the contract by one line rather than
fixing the implementation.** "A help shows both directions" is a wording-level
countermeasure and cannot constrain the choice itself.

Operational measures are needed, such as detecting contract-widening diffs in CI.
**This is not a type problem.**

### When and how an AI reads the AI Context

Designing a schema is pointless if an AI never reads it. Stating the procedure in
the equivalent of a `CLAUDE.md`, fixing on a single output command, and
guaranteeing freshness (kept out of git / a CI diff check) are all undefined.

### Dealing with the absence of training data

Verum is absent from the training data, and the documentation's implementation
examples are a single pattern, `UpdateUser` / `GetUser`. There is no worked example
of DELETE, POST, going through a service, listing, or error handling.

An AI is pulled toward the Axum idioms abundant in its training data —
`State<AppState>`, a general-purpose `save()`, variadic extractors.

- Provide at least one complete worked example per pattern
- Always put `// ❌ REJECTED` inside a rejected example's code block (do not rely
  on the heading alone)
- Run a loop from the earliest stage that records failure patterns and strengthens
  the few-shot examples

### Test strategy (a complete blank)

How a `Ctx<'req, E>` is constructed from a test. The constructor's *visibility*
is decided and measured (`pub(crate)`, `E0624`); the **sealed token is still
`proposed`** ([ADR-0006](../adr/0006-runtime-sealed-token.md)), and **the test
API's design is undecided.**

- An API with a fixed endpoint type, such as
  `verum::test::run::<UpdateUser>(req, mocks)`
- How repository mocks are supplied
- Testing the contract itself

### CLAUDE.md strategy

The framework's conventions — the attribute DSL's key names, the
everything-through-`ctx` rule, the `when` scope, the naming of per-field setters —
have to be written into every project. A minimal reference compressed to roughly
100 lines is needed.

**Write it distinguishing what is structurally enforced from what is only
convention** (what is safe to forget from what is dangerous to forget).

---

## Open problems in the AI Context

### Managing context size

400–600 tokens per endpoint, around 100k at 200 endpoints. That collides with
"tens to hundreds of tokens instead of thousands of lines". Splitting,
summarising, or fetching only the endpoints needed.

### The semantic code graph's schema

The current JSON is provisional. Whether relationships between endpoints (an
event's emitter and its subscribers) are expressed as a graph. Versioning the
schema.

### Serving it over MCP

Static JSON, or dynamic delivery from an MCP server.

---

## Open problems in the framework's design

### Escape hatches

- The declaration form (`#[escape_hatch(reason = "...")]`)
- **The recording is self-reported** — the low-level API is callable without the
  attribute. Requiring a ZST proof as an argument prevents omission structurally
- How much of the capability check is retained when dropping to the low-level
  layer
- Reconciling "freedom without chaos" with "capability-based safety" is
  **unproven** until this mechanism is designed

### The position of the service layer

`architecture-contract.md` calls handler → service → repository the permitted
path, but **no code example anywhere contains a service.** `ctx.users()` returning
the repository directly makes bypassing the service the shortest path.

Whether a service is optional or required, and if required how capabilities are
carried through it, has to be decided.

### Tidying up the design principles

`../concepts.md`'s 24 principles have no priority ordering and no adjudication
rule, and **the same argument is used in opposite directions in different
places.**

- `effect-system.md`: "forgetting to declare is indistinguishable from
  deliberately not declaring" → implicit is bad
- `read-contract.md`: "requiring explicit declaration increases omissions" →
  implicit is good

There are really six or seven distinct claims. Consolidating them with a priority
order would make collisions mechanically adjudicable.

---

## Beyond Rust

### Portability to other languages

Go has neither associated types nor ZST markers. TypeScript is structurally typed,
and literal types plus mapped types give a different representation. The design's
portability may be low.

### The difference from prior work and competitors

- Effect-system research (Koka / Eff / OCaml 5 effects)
- Prior work on capability-based security
- Session types (potentially relevant to WebSocket and streaming)
- **The difference from Nifra** — Nifra already has an AI Context and architecture
  drift detection. Verum's difference converges on the single point of "enforced by
  types". That comparison has not been worked through, while competitive claims are
  already being written
