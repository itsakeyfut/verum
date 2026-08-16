# Verum — proc-macro rules

> Verum's contract DSL is implemented with derive and attribute macros. **How
> precise the macro's errors are is how many iterations an AI needs.**
> The canon of the design is
> [`../specs/diagnostics.md`](../specs/diagnostics.md) (the three defence layers)
> and [`../specs/semantic-endpoint.md`](../specs/semantic-endpoint.md) (the
> contract DSL).

---

## 1. Three defence layers — where each error is caught

| Layer | Catches | Span precision |
|---|---|---|
| **1. proc macro (expansion time)** | `pub` fields, duplicate elements, mutation on a read-only method | **Best** — it can point at a token inside the attribute |
| **2. associated type equality bound** | Violations of `Mutates = ()` | High — the definition site of `type Mutates` |
| **3. trait bound (`Has`, `Includes`)** | Undeclared mutation, domain access violations | **Poor** — it carries no span |

### The rule: catch it at the highest layer that can

```text
can the macro catch it?  → the macro catches it (precise span, one error)
        ↓ if not
can an equality bound express it? → do that (the only route to a note with a span)
        ↓ if not
trait bound + on_unimplemented + do_not_recommend
```

**Layer 3 cannot produce a `note:` pointing at the contract declaration.** An
`on_unimplemented` note is plain text with no span, and the span rustc produces
is the definition site of the `Has` impl. Push everything that can move to layer
1.

---

## 2. What the macro must always catch

Everything below is **detected at layer 1**. These are mandatory.

| Check | Reason |
|---|---|
| A `pub` field on a domain | One of the routes that voids the whole contract, and the one a macro can catch ([`../specs/mutation-contract.md`](../specs/mutation-contract.md)). **It is not the only route** — see ledger path 21 |
| `mutates` / `creates` / `deletes` on a read-only method (`Get`, `Head`) | Including inside `when`. Types cannot check this ([`type-level.md`](./type-level.md)) |
| The same declaration at top level and inside `when` | Otherwise it surfaces as an unrelated `E0283` |
| `mutates` conflicting with `forbidden` | The only thing `forbidden` checks |
| An endpoint that is not a unit struct | Otherwise `self.pool` routes around `ctx` |
| A misspelt effect vocabulary item (`SendEmail`, …) | The infrastructure-effect vocabulary is closed ([`../specs/effect-system.md`](../specs/effect-system.md)) |

> **A nonexistent field or domain is not on this list.** A proc macro sees the
> tokens of a single item, so `#[contract(...)]` on the endpoint's unit struct
> cannot see `struct User`. See §3.

---

## 3. Emitting errors

### Give `syn::Error` a span

```rust,ignore   // fragment, not a complete item
use syn::spanned::Spanned;

return Err(syn::Error::new(
    field_path.span(),                       // the span of a token inside the attribute
    format!("no field `{}` on domain `{}`", name, domain),
));
```

**Do not reach for `proc_macro::Span::call_site()`.** The error then points at
the whole attribute and the reader cannot tell which token is wrong.

### Return several errors at once

Do not make the caller recompile after each fix.

```rust,ignore   // fragment, not a complete item
let mut errors = Vec::new();
for field in &fields {
    if let Err(e) = validate(field) { errors.push(e); }
}
if let Some(mut first) = errors.pop() {
    for e in errors { first.combine(e); }
    return Err(first);
}
```

### "did you mean" for a misspelt field

A typo in a field name is the most frequent error.

> **Computing a Levenshtein suggestion is not the macro's job.**
> `#[contract(...)]` is attached to the endpoint's unit struct, and **a proc
> macro sees the tokens of a single item only**
> ([`../specs/rust-type-model.md`](../specs/rust-type-model.md), measured), so it
> has no way to know `struct User`'s fields. The suggestion comes from **rustc's
> name resolution**; all the macro can do is expand a reference that resolves.
> Detail in [`../specs/diagnostics.md`](../specs/diagnostics.md) §A
> nonexistent field.

```text
error[E0412]: cannot find type `Statuss` in module `user`
  --> src/endpoints/user.rs:18:32
   |
18 |     mutates   = [User::name, User::statuss],
   |                              ^^^^^^^^^^^^^ help: a struct with a similar name exists: `Status`
```

### `help` always points both ways

```text
= help: add `User::status` to the contract, or remove this call
```

Offering only one direction makes **an AI mechanically widen the contract.** The
Q-C experiment demonstrated this bias
([`../specs/evaluation.md`](../specs/evaluation.md)).

---

## 4. Rules for generated code

### Use absolute paths

```rust,compile_fail
// ❌ breaks when the user writes `use verum as v;`
quote! { impl verum::Endpoint for #name { } }
```

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
// ✅
quote! { impl ::verum::Endpoint for #name { } }
```

Prefix every external path with `::`, including `::core::` and `::std::`.

### Add `#[automatically_derived]`

```rust,ignore   // fragment, not a complete item
quote! {
    #[automatically_derived]
    impl ::verum::Endpoint for #name { }
}
```

This tells rustc, the IDE and coverage tooling that the code is generated.

### Exempt generated code from lints

```rust,ignore   // fragment, not a complete item
quote! {
    #[allow(non_camel_case_types, clippy::all)]
    mod #module_name { }
}
```

So the user's lint configuration does not fire on code they did not write. **Do
not apply this to the framework's own code.**

### The shape the Domain macro must hold to (measured in T-M1-01 / #13)

The reproduction and probe table are in `spikes/domain-opacity-sqlx/`; the
verdict is in [`../specs/persistence.md`](../specs/persistence.md) §Verdict.

| Constraint | What happens otherwise |
|---|---|
| The `Repr`'s fields must **not** be fully private | `query_as!` expands into a struct literal at the *call site*, so it is `E0451` |
| Do not derive `Debug` / `Clone` / `Serialize` / `Deserialize` on the `Repr` | Ledger paths 4 and 3 reopen through the `Repr`, within the same crate |
| The domain's inner field is private, not `pub(crate)` | `u.0.email = v` compiles from anywhere in the crate |
| The domain **owns** a borrowable `Repr` | An `as_repr` returning a temporary is `E0515`. A newtype is one way to satisfy this, not the only one |

**The general form worth remembering: any derive-generated constructor that
assembles the struct inside its defining module is a forgery route.** The list
above is an enumeration, and adding a single derive opens a hole.

**The guarantee's scope is "from outside the defining module", not a type
boundary.** The macro expands into the same module as the user's `struct User`,
so an `impl` or helper written beside it sits on the permissive side. The
shortest way around an `E0616` for an AI is to move the code into the domain's
own file.

**Undecided (#18)**: a **derive cannot add an item named after its input**
(`E0428`), so `pub struct User(UserRepr)` cannot come from a derive. But the
signature `as_repr(&self) -> &Repr` **has several shapes a derive can satisfy**,
so abandoning the derive is not necessarily required. Who attaches
`#[derive(sqlx::FromRow)]` is also open — a user cannot add a derive to generated
output, and the pass-through approach has been confirmed to work.

### Avoid identifier collisions

Internal identifiers in generated code carry a `__verum_` prefix.

```rust,ignore   // fragment, not a complete item
let module = format_ident!("__verum_{}_ext", snake_case_name);
```

### Preserve and inherit spans

Give a generated type definition the span of the attribute token it came from.

```rust,ignore   // fragment, not a complete item
let mutates_ty = quote_spanned! { mutates_attr.span() =>
    type Mutates = #cons_list;
};
```

This makes the `note:` for a `Mutates = ()` violation **point at the contract
declaration** — the only route to a note with a span at layer 2
([`../specs/diagnostics.md`](../specs/diagnostics.md)).

---

## 5. Translating field references

Inside the attribute, users write `User::name` — the field name. After checking
it exists, the derive translates it into the `user::Name` marker type.

```text
User::name  →  existence check  →  user::Name (a ZST marker)
```

- It keeps the form an AI writes naturally.
- A nonexistent field is rejected with a suggestion.
- `User::name` is a real path, so IDE completion and go-to-definition work.

**Do not adopt `User.name` (dot notation).** It becomes bespoke syntax inside a
macro and rust-analyzer cannot complete it.

---

## 6. Generating cons lists

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
fn to_cons_list(items: &[TokenStream]) -> TokenStream {
    items.iter().rev().fold(quote! { () }, |acc, item| {
        quote! { (#item, #acc) }
    })
}
```

**Never generate a flat tuple** ([`type-level.md`](./type-level.md) §1).

An empty declaration becomes `()` — not a unit struct. One element is `(A, ())`,
two are `(A, (B, ()))`.

> **The type system enforces this too (T-M0-07).** `ConsList` rejects `(A, B)`
> when `B: ConsList` does not hold, so a broken generation becomes a compile
> error when `Has` is resolved. **Do not rely on that** — folding correctly is
> layer 1's job and `ConsList` is layer 3's safety net. Catching it at layer 1
> makes the error's span point at what generated it.
>
> A two-element flat tuple reads as head/tail and so **appears to work by
> coincidence**, which is why reading the generated code does not reveal it. Pin
> it with a UI test ([`test.md`](./test.md)).

### Assert the shape at the declaration site (carried over from T-M0-08)

A wrongly folded set produces a **misleading** error on the `Has` side — it says
"`(A, B)` does not contain `A`" when `A` is the first element (measured in
T-M0-08, [type-level.md](./type-level.md) §2). The wording cannot be fixed from
the `Has` side, so **emit an assertion per declaration at generation time.**

```rust
const _: () = {
    fn assert_well_formed<L: ::verum::ConsList>() {}
    fn check() { assert_well_formed::<__VerumUpdateUserMutates>(); }
};
```

A broken set then fails **once, at the declaration**, with `ConsList`'s
flat-tuple message, instead of producing unrelated errors at every use site.

### Deduplicate **before** calling `Append`

A duplicate produced while composing capabilities for a `when` scope becomes
`E0283`. The derive removes duplicates before appending.

---

## 7. Crate layout

The proc-macro crate has to be separate — a crate with `proc-macro = true`
cannot export anything else.

```text
crates/verum-macros/   ← the proc macros only
crates/verum/          ← the framework; depends on verum-macros and re-exports it
```

```rust,ignore   // needs a crate or a verum-private module this harness does not carry
// crates/verum/src/lib.rs
pub use verum_macros::{contract, endpoint, Domain, Event, Repository, Request, View};
```

Users depend on `verum` alone. They never depend on `verum-macros` directly.

### Generated code may name the `derive_facing` seals and no others

A proc macro's output is **resolved in the calling crate**, so it cannot reach
`pub(crate) mod private` (`E0603`, measured). M2 therefore has to introduce a
`#[doc(hidden)] pub mod __private` re-export.

**The only seals that may be exposed there are the `derive_facing` ones.**

| Module | Contents | Exposed in M2? |
|---|---|---|
| `private` | `SealedConsList` / `SealedIndex` / `SealedHas` / `SealedAppend` / `SealedLookup` — verum implements these itself for `()` and tuples. **A derive never writes one of these impls** | **No. Permanently `pub(crate)`** |
| `derive_facing` | `SealedIncludes`, joined in M2 by `SealedEndpoint` / `SealedField` / `SealedCondition` | Yes, as `__private` |

**Exposing both at once reopens ledger paths 14a–14e simultaneously** — during
T-M0-09's review that change was made and forged membership was confirmed to
compile. `structural_seals_should_not_be_reachable_through_the_derive_facing_module`
in `crates/verum/src/sealed.rs` enforces the boundary mechanically, and
`compile_fail/sealed_derive_facing_module_is_private.rs` pins the current
non-exposure.

> **Exposing `Endpoint` is worse than exposing `Includes`.** `Endpoint` carries
> `Reads` / `Mutates` / `Creates` / … as associated types, so the moment
> `SealedEndpoint` becomes nameable, **a user crate can declare any capability
> set directly** — stronger than forging `Has`, which only queries a set someone
> else chose. That is ledger path 12, and it outweighs path 13 (`Includes`).
> When `__private` arrives in M2, re-verify both 12 and 13.

### Pin the version

```toml
verum-macros = { version = "=0.1.0", path = "../verum-macros" }
```

Generated code calls `::verum::` internals, so a version skew breaks it.

---

## 8. Collecting contracts with `inventory`

Each endpoint's contract is collected at compile time and used for the AI Context
output and the route table.

```rust,ignore   // fragment, not a complete item
quote! {
    ::verum::inventory::submit! {
        ::verum::ContractEntry {
            endpoint: #name_str,
            method: #method_str,
            path: #path_str,
            contract_json: #json_str,     // embedded as a static string
        }
    }
}
```

- **The macro builds the contract JSON while expanding and embeds it as a static
  string.** Never assemble it at runtime ([`perf.md`](./perf.md), lean runtime).
- `enforcement` and `unverified_boundaries` are embedded at the same time
  ([`../specs/ai-context.md`](../specs/ai-context.md)).

---

## 9. Testing

### Pin the macro's errors with UI tests

```text
crates/verum-macros/tests/ui/no_such_field.rs
crates/verum-macros/tests/ui/no_such_field.stderr
```

**The error wording is a specification.** Change it deliberately, and update the
`.stderr` in the same change ([`test.md`](./test.md)).

### Check the expansion

```bash
cargo expand --test contract_expansion
```

Snapshot-comparing expanded code with `macrotest` is acceptable, but the UI tests
— the error side — take priority.

---

## 10. Dependencies

| Crate | Purpose |
|---|---|
| `syn` | Parsing (`features = ["full", "extra-traits"]`) |
| `quote` | Code generation |
| `proc-macro2` | Span manipulation |

- **`darling` is optional.** Consider it if attribute parsing gets complex — but
  **fall back to hand-written parsing if it costs control over error spans.**
  Error precision comes first.
- Dependencies feed straight into compile time, so measure the effect with
  `cargo build --timings` before adding one.

---

## Never do this

- ❌ Emit errors carelessly with a `call_site()` span — it points at the whole
  attribute.
- ❌ Write `verum::` in generated code. Use `::verum::`.
- ❌ Leave to the type checker something layer 1 could catch.
- ❌ Offer `help` in one direction only — an AI will widen the contract.
- ❌ Generate a flat tuple.
- ❌ Assemble the contract JSON at runtime; make it a static string at expansion.
- ❌ Change error wording without checking it — the UI tests pin it.
