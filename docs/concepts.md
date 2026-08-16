# Verum — concepts

The project's philosophy, vision and design principles.

For the technical specifications see [`docs/specs/`](./specs/README.md); the
reasoning behind each decision is in [`docs/adr/`](./adr/README.md).

---

## 1. Project vision

Assuming an AI implements the web application, build **a web framework in which
an endpoint's semantics, mutability and side effects are expressed in a strong
type system, so that an AI can understand the exact contract without reading
large amounts of the implementation.**

Not merely "a web framework an AI finds easy to read", but:

> **A web framework in which an AI produces correct web architecture, and in
> which an implementation deviating from the contract is detected and rejected by
> the compiler or by static analysis.**

### Fundamental philosophy

- Rather than letting an AI write code freely, **narrow the space of correct
  designs**
- Rather than guiding an AI by convention alone, **constrain its implementation
  with a typed contract**
- Do not let meaning rest on comments, READMEs and naming
- **Guarantee an endpoint's metadata itself in the type system**
- State as typed metadata, as far as possible, what would otherwise only be
  learnable by reading the code
- Use the type system as **an information-compression device aimed at an AI**

---

## 2. Core philosophy

In one sentence:

> **A high-performance AI-first web framework in which an AI can understand an
> endpoint's meaning without exploring the whole codebase, and in which it is free
> to implement — while implementations that deviate from the intent are refused by
> the type system, the effect system, the capability system and the architecture
> contract.**

More briefly:

> **Freedom without chaos, semantics without comments.**

And ultimately:

> **Not "have an AI write the code", but "build a world of web applications in
> which it is hard for an AI to write anything but correct code".**

---

## 3. Design principles

> A consolidation of the old §15 Design Principles and §50 Updated Core
> Principles.

### AI as primary developer

1. **AI first**
   - Design with the AI as the primary developer.

2. **AI Context is a first-class artefact**
   - Treat the semantic context an AI reads as a first-class artefact of the
     framework.

3. **Token-efficient context**
   - Supply the semantics an AI needs in few tokens.

### Contract over convention

4. **Convention over configuration**
   - Inherited from Rails.

5. **Contract over convention**
   - Not convention alone but a typed contract.

6. **Semantics over syntax**
   - Express an endpoint's meaning, not just its HTTP method and function name.

7. **Semantic endpoint**
   - An endpoint expresses domain, effects, mutation and capability — not only an
     HTTP route.

### Types are the source of truth

8. **Types are authoritative**
   - Trust types and contracts, not comments.

9. **Metadata is executable truth**
   - Semantic metadata is not documentation but a contract that constrains the
     implementation.

10. **Comments are non-authoritative**
    - A comment is supplementary information, not the specification's source of
      truth.

11. **Comments are not contracts**
    - The goal is that neither an AI nor a human is left guessing without them.

12. **Single source of truth**
    - AI Context, documentation, OpenAPI and IDE information are all generated
      from the same contract.

13. **Self-describing codebase**
    - The codebase itself carries typed semantics.

14. **A contract must be trustworthy**
    - Divergence between the metadata shown to an AI and the implementation is
      detected statically.

### Effects and capabilities

15. **Explicit effects**
    - Do not hide effects.

16. **Fine-grained effects**
    - Express them at a granularity an AI can act on, not as one coarse `IO`.

17. **Capability *and* permission checks**
    - A capability is **an endpoint's upper bound of ability**, a separate concept
      from the caller's permissions.
    - **Authorisation is always needed separately.** A capability is not a
      substitute for it.
    - Do not conflate "what can this endpoint do" (compile time) with "what is
      this caller allowed to do" (run time). Detail in
      [`specs/capability-system.md`](./specs/capability-system.md).

18. **Capability-based safety**
    - Rather than explaining that something must not be used, create a type-level
      state in which "calling it does not compile".
    - But **there is a range the type check does not reach.** It is all listed in
      [`specs/unverified-boundaries.md`](./specs/unverified-boundaries.md) and
      emitted in the AI Context.

### Freedom and performance

19. **Freedom without chaos**
    - Do not take away freedom over middleware and the lower layers.

20. **Roads to low-level**
    - Provide a typed road to the lower layers too.

21. **Escape hatch**
    - Drop down to raw HTTP, the network or the runtime when necessary. Keep 80–90%
      on strong rails while providing a low-level API for special cases — but state
      the escape hatch to the AI and the IDE.

22. **Compile-time first**
    - Verify the semantic contract at compile time as far as possible.

23. **Runtime lean**
    - Do not turn rich metadata aimed at an AI into runtime overhead.

24. **High performance**
    - Target Axum-class performance, and investigate Actix Web-class where
      possible.

---

## 4. Prior art

### Ruby on Rails

Rails's convention over configuration is important prior art.

Rails:
- Narrows the search space by convention
- Standardises structures such as MVC, REST and Active Record
- A predictable architecture also favours an AI agent

The difference here:
- Rails: **convention makes it easier for an AI to guess**
- This project: **convention plus a typed effect contract makes it hard for an AI
  to produce anything but a correct implementation**

### Goa

Goa:
- Design-first
- Typed contract
- A DSL
- Code generation

What is taken from it:
- Turning an API contract into generated code and types
- Declaring a service's meaning before implementing it

### The exact difference from Goa

**The framing "types are authoritative vs. an external file is authoritative"
does not hold.** The contents of Verum's `#[contract(...)]` are not Rust type
syntax either but a token stream a proc macro interprets, and the types are its
output. The authority structure of the declaration is the same as Goa's.

Two differentiators do hold:

1. **What the contract covers** — not only the API contract but **state
   mutation, external effects, conditional effects, capabilities and
   architecture**
2. **The locality of errors** — a violation comes back as **a compile error
   pointing at the declaration** (Goa's verification finishes at generation time)

Note that error declaration and OpenAPI generation, which Goa already covers, are
undecided or deferred to the Full PoC in Verum. **On the API-contract axis Goa is
currently ahead**, and that should be kept in view.

### Igniter.js

An AI-native TypeScript framework.
- Predictable architecture
- Explicit structure
- An AI-friendly codebase

What is taken from it:
- A structure an AI finds easy to understand
- Convention and predictability

This project goes further, into typed effect and mutation contracts.

### Nifra

A TypeScript framework built around an AI-edited codebase.
- AI Context
- Scaffolding
- Validation
- Architecture drift detection

What is taken from it:
- Providing structured context so an AI can operate on the codebase
- Architecture validation

### AI agent frameworks

Google ADK Go, Microsoft Agent Framework and the like point at building
applications that embed AI.

This project points the other way:
- **It is not a framework for building AI**
- **It is a framework for an AI to implement web applications correctly**

---

## 5. Core differentiation

### An endpoint is a semantic contract, not an HTTP function

Ordinarily:

```rust,ignore   // needs a macro that arrives in M2
#[put("/users/{user_id}")]
async fn update_user(...) -> Result<User>
```

From that alone, without reading the endpoint's body, none of the following is
knowable:

- what it changes
- what it reads
- whether it writes to the database
- whether it calls an external service
- whether it emits an event
- what differs under which conditions

Here, all of that is expressed on the endpoint itself. And what matters is that
this information is not a comment but **guaranteed by the type system.**

How it is expressed concretely is in
[`specs/semantic-endpoint.md`](./specs/semantic-endpoint.md).

---

## 6. Positioning

> A consolidation of the old §17 Potential Project Positioning and §46 Framework
> Positioning.

Not merely:

> an AI-friendly web framework

but:

> **an AI-native web framework**

or:

> **a semantic / effect-aware web framework**

### Against the existing frameworks

```text
Hyper / Tower
    ↓
HTTP / middleware foundation

Axum
    ↓
composable web framework

Actix Web
    ↓
high-performance web framework

Rocket
    ↓
declarative / compile-time-checked web framework

Loco
    ↓
Rails-like full-stack framework

Pavex
    ↓
compile-time dependency injection / architecture

Verum
    ↓
AI-first semantic web framework
```

### What is distinctive here

Treating all of

```text
HTTP
+
domain semantics
+
mutation
+
conditional effects
+
external effects
+
capabilities
+
architecture
+
AI Context
```

as typed metadata.

### The philosophy, stated once

> **Reduce how much of a web application's code an AI has to read, so that an
> endpoint's meaning, mutability, side effects, capabilities and architecture are
> understandable from typed metadata — and guarantee statically that the metadata
> and the implementation agree.**

---

## 7. Comments are not the source of truth

This framework **does not treat comments as a trustworthy source for the
specification.**

The overriding principle:

> **Types and semantic metadata are authoritative. Comments are supplementary.**

The goal is that an AI, a human implementer, an IDE and static analysis can all
understand an endpoint's meaning with no comments present at all.

### The trust ordering

Conceptually, trust runs in this order:

1. Type / contract
2. Semantic metadata
3. Static analysis / inferred semantics
4. Implementation
5. Generated documentation
6. Human comments

Comments are not banned. They may be used as supplementary explanation, but **they
are never the specification's authority.**

### An example

```rust,ignore   // fragment, not a complete item
/// This endpoint only updates the user's name.
fn update_user(...) {
    user.name = new_name;
    user.email = new_email;
}
```

Here the comment is not trusted; what mutations are actually permitted is decided
by the types and the contract.

---

## 8. A self-describing codebase

The end goal is a state in which, through the framework, the codebase itself
carries typed semantic metadata.

```text
Codebase
├── types
├── contracts
├── effects
├── capabilities
├── architecture
└── state transitions
```

From which the following can be generated:

```text
              type / contract
                     │
        ┌────────────┼────────────┐
        ↓            ↓            ↓
       AI       documentation    IDE
        │
        ↓
   implementation
```

Rather than maintaining these separately, **the semantic contract is the single
source of truth.**

What is generated is detailed in
[`specs/ai-context.md`](./specs/ai-context.md).

---

## 9. The token-efficiency goal

Remove the need for an AI to explore handler → service → repository → model →
event → middleware to understand an endpoint.

Conventionally:

```text
endpoint
  ↓
handler
  ↓
service
  ↓
repository
  ↓
model
  ↓
event
  ↓
middleware
  ↓
a great deal of code to explore
```

Here:

```text
endpoint
  ↓
semantic contract
  ↓
explore only what is needed
```

### The goal

> **Instead of reading thousands of lines of code, read a semantic contract of
> tens to hundreds of tokens first.**

The AI uses the contract as the entry point and explores only the code it
additionally needs.

### This claim has to distinguish between situations (unverified)

| Task | Budget |
|---|---|
| **Surveying** many endpoints | Positive. Reading the contracts is enough |
| **Editing** one endpoint | **Possibly negative.** Both the contract and the implementation have to be read, plus knowledge of the framework's conventions |

Real AI coding is mostly the latter. On top of that:

- The AI Context is roughly 400–600 tokens per endpoint, around 100k at 200
  endpoints ([`specs/ai-context.md`](./specs/ai-context.md) itself records size
  management as unresolved)
- Verum has roughly 40 concepts (Axum 8–10, Rails 15–20), and **because they are
  absent from the training data they have to be loaded into context every
  session**

**Until the break-even point is produced as a number, this claim should not be
made unconditionally.** Under consideration is repointing the objective from
"fewer tokens" to **"make the compiler the AI's feedback loop and stop contract
violations before they run"**. See
[`specs/research-questions.md`](./specs/research-questions.md) Q-B.

The concrete contract format is in
[`specs/ai-context.md`](./specs/ai-context.md).

---

## 10. AI coding as a first-class design constraint

When designing the framework API, treat the following as first-class metrics
alongside human ergonomics.

- AI discoverability
- AI context size
- AI ambiguity
- AI error rate
- AI exploration cost
- Specification violation rate
- Unexpected behaviour rate

The metrics are detailed in [`specs/evaluation.md`](./specs/evaluation.md).

---

## 11. Freedom without chaos

"Freedom" here does not mean restricting access to the lower layers.

### What is not wanted

```text
code can only be written where the framework says it may
```

— an excessively opinionated design.

### What is wanted

```text
middleware (outside the endpoint)
      ↓
high-level API
      ↓
service
      ↓
repository
      ↓
runtime
      ↓
raw HTTP / network
```

Every layer is reachable. But every layer gets **a typed road.**

> **Freedom without chaos.**

Not taking freedom away, but **paving the roads freedom travels on.**

> **Caution: this reconciliation is unproven today.** The only basis for
> distinguishing "an unregistered bypass" from "an escape hatch" is whether it is
> declared, and the declaration mechanism (`#[escape_hatch]`) has not been designed
> yet. The recording is self-reported, and forgetting the attribute means nothing
> is recorded.
>
> Because no component connects principle 18 (capability-based safety) to
> principle 21 (escape hatch), this section is **a goal, not an achieved
> property.** See [`specs/research-questions.md`](./specs/research-questions.md).

---

## 12. Performance philosophy

Being AI-first is not a reason to sacrifice runtime performance.

Semantic metadata is consumed at compile time as far as possible.

```text
semantic contract
        ↓
compile time
        ↓
validation
        ↓
optimisation
        ↓
lean runtime
```

Ideally the runtime carries no significant overhead from metadata that exists for
an AI's benefit.

The performance targets are detailed in
[`specs/performance.md`](./specs/performance.md).

---

## 13. Naming

### The project's name: Verum

**Verum** — the truth, that which is true.

Not "trust the code an AI produced", but a guarantee that the code is
*verum* — true.

### Concepts considered

```text
Intent
Contract
Semantic
Effect
Mutation
Capability
Proof
Invariant
Axiom
Verity
Pact
Rail
```

#### Intent

What the AI is trying to achieve.

```text
AI
 ↓
intent
 ↓
implementation
```

#### Pact

The contract between AI, framework and code.

```text
endpoint pact
effect pact
mutation pact
architecture pact
```

#### Axiom

The invariants that must not be broken.

```text
axiom
 ↓
invariant
 ↓
proof
```

#### Verity

The idea of guaranteeing correctness through types and contracts rather than
simply trusting AI-generated code.

#### Rail / Path / Way

The idea of laying a road so the AI does not get lost, rather than taking its
freedom away.

Reconsidered at formal naming, given the confusion with Rails among other things.

### Naming strategy

Starting development under a working name is acceptable.

The recommended process:

```text
prototype
 ↓
hello world
 ↓
CRUD
 ↓
a TODO app
 ↓
semantics / effects / capabilities settle
 ↓
formal naming
```

Rather than fixing the name to the design philosophy too early, decide the formal
name once the TODO app is complete and the framework's essence is visible.
