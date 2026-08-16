# Verum — `unsafe` code rules

> Verum sits on top of **safe abstractions** (tokio / hyper / tower / http), so
> **it writes no `unsafe` of its own.** Related: [rust.md](./rust.md).

## Policy

- **Every crate is safe Rust.** Neither `verum` nor `verum-macros` is expected to
  contain hand-written `unsafe`.
- What Verum handles is **types and HTTP dispatch**. No FFI, no raw pointer work.
- If `unsafe` ever becomes unavoidable, **isolate it in a dedicated module** and
  do not let it escape through a `pub` function.
- Every `unsafe` block and `unsafe impl` carries a **`// SAFETY:`** comment
  saying why it is sound.

---

## Enforced by lints

```toml
# root Cargo.toml
[workspace.lints.rust]
unsafe_code = "forbid"
unsafe_op_in_unsafe_fn = "warn"

[workspace.lints.clippy]
undocumented_unsafe_blocks = "warn"
```

- `#![forbid(unsafe_code)]` applies to every crate, set once through workspace
  lints.
- `forbid` is a wall, but it is mainly a **tripwire**: nothing slips in quietly,
  and relaxing the policy always shows up in a diff and in review.

---

## Why Verum does not need `unsafe`

| Common reason to reach for it | What Verum uses instead |
|---|---|
| Type erasure, vtable manipulation | `Box<dyn Trait>` and RPITIT are enough ([`rust.md`](./rust.md)) |
| ZST optimisation | `PhantomData` suffices; no `unsafe` involved |
| Type-level computation | Trait resolution only. No runtime type manipulation |
| Buffer manipulation | The safe APIs of `bytes` / `http-body-util` |
| Adding `Send` / `Sync` | Satisfied structurally. Never implemented by hand |

### Do not write `unsafe impl Send / Sync`

`Ctx<'req, E>` has to be `Send` (hyper's multi-thread runtime requires it), but
**that is derived automatically when its contents are `Send`.**

```rust,compile_fail
// ❌ hand-written impls are forbidden
unsafe impl<'req, E> Send for Ctx<'req, E> {}

// ✅ make the contents `Send`. If it is not derived, revisit the design
```

When it is not derived, an `Rc` or a raw pointer has crept in. **Removing that is
the correct fix.**

---

## SAFETY comments (for the exceptional case)

```rust,ignore   // fragment, not a complete item
// SAFETY: T is #[repr(transparent)] over U, so the layouts are identical.
let u: &U = unsafe { &*(t as *const T as *const U) };
```

Before writing anything like this, check whether a **safe alternative** avoids it
— `bytemuck`'s derives, a standard API, or a design that does not need
`transmute`. That is the first choice, not the fallback.

---

## Escalation, if `unsafe` ever becomes necessary

Work down this list in order.

1. **Check whether a safe API or safe crate avoids it.** In Verum's territory
   this resolves almost every case.
2. **If it is unavoidable, isolate it in a dedicated module.** Opt in locally
   there with `#[allow(unsafe_code)]` plus `// SAFETY:`, and leave callers under
   `forbid`. **Do not let `unsafe` reach the public API.**
3. **Last resort: remove `#![forbid(unsafe_code)]` from the crate.** This weakens
   a guarantee, so it is a deliberate, reviewed decision, and this document is
   updated in the same change under the rule-change policy in
   [README.md](./README.md).

The constants across all three: every `unsafe` carries `// SAFETY:`, no `unsafe`
reaches the public API, and the change is reviewed.

---

## `unsafe` in generated code

**A derive must never generate code containing `unsafe`.**

A user's crate may well set `#![forbid(unsafe_code)]`, and generated `unsafe`
would break their build.

```rust,compile_fail
// ❌ a derive must not emit this
unsafe impl Send for __VerumGenerated { }
```

`forbid` applies to generated code too, so **this is enforced by the compiler
rather than by convention.** It is worth keeping in mind at design time all the
same.
