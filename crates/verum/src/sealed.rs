//! The supertraits every capability-gating trait carries.
//!
//! Sealing exists because of how an AI responds to a trait-bound error. Verum
//! shows a lot of them on purpose — that is the product — and the first repair
//! reached for is to write the missing impl. `impl Includes<Order> for User {}`
//! compiles: `User` is a local type, so the orphan rule allows it. One line
//! removes the guarantee, `cargo build` succeeds, and nothing reports it.
//!
//! See `docs/rules/api-surface.md` §2 and `docs/specs/unverified-boundaries.md`
//! paths 12–14.

/// Private by construction: a downstream crate cannot name this module, so it
/// cannot write the impls every sealed trait requires.
///
/// The module is load-bearing, not stylistic. A bare `pub(crate) trait` used as
/// the supertrait of a public trait is rejected — `trait ... is more private
/// than the item ...` (`private_bounds`). Nesting `pub` traits inside a
/// `pub(crate)` module is what makes the visibilities line up.
///
/// # One seal per sealed trait
///
/// An earlier shape used a single `Sealed<Args>` keyed by a discriminator. It
/// was abandoned on measurement: **rustc's sealed-trait help enumerates every
/// impl of the seal regardless of its arguments**, so each sealed trait's error
/// listed every *other* sealed trait's implementors. Trying to write
/// `impl Includes<Order> for User` suggested `()`, `(H, T)`, `Here` and
/// `There<I>` as the fix. That list grows with each new sealed trait.
///
/// Separate seals keep each error to its own implementors. The cost — an
/// annotation per seal, which is easy to forget — is why they are declared
/// through [`seal!`] rather than by hand.
/// Declares a seal.
///
/// Both seal modules below use this; it is defined at file scope so they can.
///
/// The diagnostic is emitted here, so a seal declared through this macro
/// always carries one. That matters because the annotation is what stops a
/// raw trait-bound error reaching the reader, and a seal added in a hurry is
/// exactly where it would be omitted.
///
/// The macro does **not** by itself make a hand-written seal impossible — a
/// plain `pub trait SealedX {}` in this module compiles and passes the lint
/// table (measured). What forbids it is
/// [`tests::seals_should_only_be_declared_through_the_macro`], which reads
/// **every source file** and rejects a `pub trait` declared inside a
/// non-`pub` module — that being what "used as a seal" means structurally.
/// (Until #32 it read only this file, so a hand-rolled seal anywhere else
/// passed unnoticed.) Only doc comments pass through `$attr`, so a caller
/// cannot override the mandated diagnostic either.
///
/// The wording is deliberately generic: it fires for hand-written impls of
/// any sealed trait, including ones that have nothing to do with contracts.
/// Trait-specific guidance belongs on the sealed trait itself, which fires
/// on a different unsatisfied bound and composes with this.
macro_rules! seal {
    ($(#[doc = $doc:expr])* $name:ident $(<$($param:ident),+ $(,)?>)?) => {
        $(#[doc = $doc])*
        #[diagnostic::on_unimplemented(
            message = "`{Self}` cannot implement a sealed Verum trait",
            label = "not sealed by Verum",
            note = "Verum's sealed traits are implemented by Verum itself and by its derive macros. Writing the impl by hand grants something no declaration authorises, and nothing would report the difference."
        )]
        pub trait $name $(<$($param),+>)? {}
    };
}

pub(crate) mod private {
    seal! {
        /// Seals [`crate::ConsList`].
        SealedConsList
    }

    seal! {
        /// Seals [`crate::Index`].
        SealedIndex
    }

    seal! {
        /// Seals [`crate::Has`].
        ///
        /// Parameterised by both the element and the index, mirroring `Has`
        /// itself: sealing on `Self` alone would let one derive-generated impl
        /// unlock membership for elements the contract never declared.
        SealedHas<T, Idx>
    }

    seal! {
        /// Seals [`crate::Append`].
        ///
        /// The stakes are higher here than for a predicate. `Append` carries
        /// `type Out`, so a forged impl does not merely assert something — it
        /// *chooses the composed capability set*. Measured: with this seal's
        /// recursive impl written unconditionally, a downstream crate can name
        /// any `Out` it likes for a list whose tail is not a cons list.
        SealedAppend<B>
    }

    seal! {
        /// Seals [`crate::Lookup`].
        ///
        /// Same `type Out` exposure as [`SealedAppend`], and the same
        /// requirement that the recursion be mirrored.
        SealedLookup<K, Idx>
    }
}

/// Seals a **derive** must be able to satisfy — and therefore the only ones that
/// M2 has to expose.
///
/// This module exists because of a latent Critical found reviewing T-M0-09.
/// `docs/rules/api-surface.md` §2 already records that proc-macro output is
/// resolved in the *caller's* crate, so it cannot name `pub(crate) mod private`
/// (E0603, measured) — and that M2 therefore has no choice but to re-export a
/// `#[doc(hidden)] pub mod __private`. §2 calls that "the moment the seal gets
/// weaker".
///
/// The part nobody had noticed: with every seal in one module, that single change
/// would make **all** of them nameable, reopening ledger paths 14a–14e at once. I
/// simulated it and a false membership compiled downstream.
///
/// So the seals are split by whether a derive ever needs to satisfy them:
///
/// - [`private`] — **structural** seals (`SealedConsList`, `SealedIndex`,
///   `SealedHas`, `SealedAppend`, `SealedLookup`). Verum implements these itself,
///   over `()` and tuples. No derive writes such an impl and none is planned, so
///   this module stays `pub(crate)` **permanently** and M2 cannot weaken it.
/// - `derive_facing` — seals a derive must satisfy per declaration. Only
///   `SealedIncludes` today; `SealedEndpoint` / `SealedField` / `SealedCondition`
///   join it in M2.
///
/// Splitting now costs two modules. Splitting after M2 would be a breaking change
/// to whatever `__private` had already exposed.
pub(crate) mod derive_facing {
    seal! {
        /// Seals [`crate::Includes`].
        ///
        /// Parameterised by the domain, so the seal covers the *relationship*
        /// rather than the type. Sealing on `Self` alone would let one
        /// derive-generated impl unlock every other domain — see
        /// `docs/rules/api-surface.md` §2.
        SealedIncludes<D>
    }
}

#[cfg(test)]
mod tests {
    /// Every `.rs` under `src/`, recursively.
    ///
    /// The recursion is the point. This helper exists because the same scope bug
    /// shipped three times: first the checks read one file (`typelevel.rs`), then
    /// "all of `src/`" was implemented as `read_dir("src")` — which does not
    /// descend. `CLAUDE.md`'s own module plan is written with directories
    /// (`runtime/`), so `src/effect/mod.rs` is the expected shape, and an
    /// unannotated wide-open seal impl there passed the entire suite while
    /// granting a real downstream forgery (measured).
    ///
    /// Each fix had addressed the instance rather than the class. The class is:
    /// **a guard must not depend on where code happens to live.**
    fn source_files() -> Vec<std::path::PathBuf> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir)
                .expect("unreadable directory")
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|x| x == "rs") {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        walk(std::path::Path::new("src"), &mut out);
        out.sort();
        out
    }

    /// The seal trait names, taken from the `seal!` invocations themselves.
    ///
    /// Derived rather than hardcoded, and *not* matched by the substring
    /// `"Sealed"`. A seal is whatever `seal!` declares, so
    /// `seal! { VerumOnlyGuard }` is a seal too — measured: with a name-based
    /// scan, a blanket `impl<T> private::VerumOnlyGuard for T {}` needed no
    /// marker and the suite stayed green.
    ///
    /// **Every source file, not `include_str!("sealed.rs")`** (`docs/rules/test.md`
    /// §9 rule 7). Reading one file is correct *by placement* today, because
    /// `macro_rules! seal` is not exported and so is textually confined here —
    /// and that is exactly the dependency the rule forbids. The day someone adds
    /// `pub(crate) use seal;`, the name set silently stops covering the new
    /// seals, and `every_seal_impl_should_declare_whether_it_mirrors_its_trait`
    /// stops checking their impls with nothing going red.
    fn declared_seals() -> Vec<String> {
        let mut names = Vec::new();
        for path in source_files() {
            let src = std::fs::read_to_string(&path).expect("unreadable source");
            names.extend(seal_names_in(&src));
        }
        assert!(
            names.len() >= 6,
            "found only {} seal declarations — the `seal! {{` scan has drifted",
            names.len()
        );
        names
    }

    /// The `seal!`-declared names in one source, as a pure function so it can be
    /// unit-tested against a known input instead of only against the real tree.
    fn seal_names_in(src: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut in_seal = false;
        for line in src.lines() {
            let t = line.trim();
            if t == "seal! {" {
                in_seal = true;
                continue;
            }
            if in_seal {
                if t.starts_with("///") || t.is_empty() {
                    continue;
                }
                if t != "}" {
                    names.push(t.split('<').next().unwrap_or(t).trim().to_string());
                }
                in_seal = false;
            }
        }
        names
    }

    /// The names `lib.rs` re-exports, i.e. the traits a downstream crate can
    /// actually name.
    ///
    /// **This is the property that matters, and deriving it is the whole point.**
    /// An earlier version of this guard asked "is this `pub trait` inside a
    /// non-`pub` module?" — which sounds equivalent and is not. `lib.rs` declares
    /// `mod domain; mod sealed; mod typelevel;`, all private, so *every* file
    /// scope in this crate is already inside a non-`pub` module. What makes
    /// `ConsList` and friends public is the `pub use` below, nothing else.
    /// Measured: six working seals — a private module in its own file, a
    /// file-scope `pub trait`, `pub(super) mod`, and three more — passed that
    /// predicate green, and a downstream crate compiled the forgeries.
    fn reexported_names() -> Vec<String> {
        let src = include_str!("lib.rs");
        let mut out = Vec::new();
        for line in src.lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub use ") else {
                continue;
            };
            for part in rest
                .trim_end_matches(';')
                .split(['{', '}', ',', ':'])
                .flat_map(|p| p.rsplit("::"))
            {
                let p = part.trim();
                if !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    out.push(p.to_string());
                }
            }
        }
        out
    }

    /// The trait name a `pub trait` line declares, if the line declares one.
    ///
    /// Returns `None` for the `seal!` template (`pub trait $name …`), because
    /// `$name` is not an identifier. That is deliberate: an earlier version
    /// skipped any line *containing* the text `$name`, and a seven-character
    /// trailing comment — `pub trait Forgeable {} // $name` — turned the whole
    /// guard off (measured). Reading the declaration instead of the line leaves
    /// nothing for a comment to influence.
    fn declared_pub_trait(line: &str) -> Option<&str> {
        let rest = line.trim_start().strip_prefix("pub trait ")?;
        let name: &str = rest
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()?;
        let mut chars = name.chars();
        match chars.next() {
            Some(c) if c.is_alphabetic() || c == '_' => Some(name),
            _ => None,
        }
    }

    /// The hand-rolled seals in one source, as a pure function.
    ///
    /// **A seal is a `pub trait` that is neither declared through [`seal!`] nor
    /// re-exported by `lib.rs`.** Everything reachable downstream is in
    /// [`reexported_names`]; everything with a diagnostic floor is in
    /// [`declared_seals`]. A `pub trait` in neither set is unreachable from
    /// outside the crate *and* carries no floor — which is exactly what makes a
    /// trait usable as a seal, and is the property `docs/rules/api-surface.md`
    /// §2 cares about.
    ///
    /// Both sets come from the **declaration side** — `seal!` invocations and
    /// `pub use` lines — as `docs/rules/test.md` §9 **rule 7** requires. No
    /// module nesting is tracked, no column is assumed, and no substring of the
    /// line can influence the verdict.
    fn handrolled_seals_in(name: &str, src: &str, allowed: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for (i, line) in src.lines().enumerate() {
            let Some(declared) = declared_pub_trait(line) else {
                continue;
            };
            if allowed.iter().any(|a| a == declared) {
                continue;
            }
            out.push(format!("{name}:{}: {}", i + 1, line.trim_start()));
        }
        out
    }

    /// Impl headers, with rustfmt's line wrapping undone.
    fn impl_headers(src: &str) -> Vec<(usize, String)> {
        let mut items = Vec::new();
        let mut buf = String::new();
        let mut start = 0usize;
        for (i, line) in src.lines().enumerate() {
            let t = line.trim_start();
            if buf.is_empty() {
                if !t.starts_with("impl") {
                    continue;
                }
                start = i;
            }
            buf.push(' ');
            buf.push_str(t);
            if t.ends_with('{') || t.ends_with(';') || t.ends_with("{}") {
                items.push((start, std::mem::take(&mut buf)));
            }
        }
        items
    }

    /// Every seal must go through `seal!`, because that is what guarantees the
    /// `on_unimplemented` floor. A hand-written `pub trait SealedX {}` compiles
    /// and passes `-D warnings` while silently dropping the floor — measured.
    ///
    /// Scans every source file, not just this one: a bare seal declared inside a
    /// `pub(crate) mod` in another file would otherwise skip the floor entirely.
    ///
    /// **It did exactly that until #32.** The loop below walked `source_files()`
    /// and then kept only `sealed.rs`, so the traversal was decorative and this
    /// comment described an intent the code did not have. Measured: a
    /// `pub(crate) mod handrolled { pub trait MySeal {} }` in `domain.rs`, with a
    /// wide-open `impl<T> handrolled::MySeal for T {}`, left the suite at 12/12.
    ///
    /// That is the *fourth* instance of one class — `include_str!` on one file,
    /// then a non-recursive `read_dir`, then a bare seal in a subdirectory, then
    /// this. `docs/rules/test.md` §9 **rule 7** is where the class is written
    /// down: *everywhere a set of files is enumerated, recurse into directories,
    /// derive the target names from the declaration side, and forbid aliases.*
    #[test]
    fn seals_should_only_be_declared_through_the_macro() {
        let files = source_files();
        // Assert the work happened, not that no error appeared. Without this the
        // guard passes when the scan reads nothing — measured: filtering
        // `source_files()` to a nonexistent extension left it green, and only a
        // *sibling* guard's floor noticed. `docs/rules/test.md` §9's headline.
        assert!(
            files.len() >= 4,
            "only {} source files scanned — the walk has drifted",
            files.len()
        );
        // Anything with a diagnostic floor, plus anything a downstream crate can
        // already name. Both derived, neither hardcoded.
        let mut allowed = declared_seals();
        allowed.extend(reexported_names());

        let mut strays: Vec<String> = Vec::new();
        for path in &files {
            // `source_files()` walks `src/`. A `#[path = "../gen/x.rs"] mod x;`
            // puts a module's body somewhere the walk never opens, so assert the
            // escape hatch is unused rather than trying to follow it. Cheap, and
            // it fails loudly the day `build.rs` / `OUT_DIR` code arrives.
            // Matched at the start of a trimmed line, not as a substring: the
            // substring form reports this very comment, which is the
            // reporting-on-itself hazard this file records twice already.
            let src_check = std::fs::read_to_string(path).expect("unreadable source");
            if let Some(line_no) = src_check
                .lines()
                .position(|l| l.trim_start().starts_with("#[path"))
            {
                panic!(
                    "{}:{}: `#[path]` moves a module body outside the scanned tree",
                    path.display(),
                    line_no + 1
                );
            }
            let src = std::fs::read_to_string(path).expect("unreadable source");
            strays.extend(handrolled_seals_in(
                &path.display().to_string(),
                &src,
                &allowed,
            ));
        }
        assert!(
            strays.is_empty(),
            "seal declared outside `seal!`, so it carries no diagnostic:\n{}",
            strays.join("\n")
        );
    }

    /// A seal that is *more permissive* than the trait it seals is a forgery hole,
    /// exactly the size of the difference. That shipped three times — #6 (missing
    /// type argument), #8 (missing recursion), #9 (missing bound on a parameter
    /// that does not appear in `Self`) — and each time the response was to widen a
    /// prose rule in `docs/rules/api-surface.md` §2.
    ///
    /// The rule kept failing because it was **convention**: #9 complied with §2 as
    /// written and still shipped two open routes. So the requirement is mechanical
    /// here. Every seal impl must carry `SEAL-EXACT`, or `SEAL-DIFF` with a
    /// justification and a `fixture:` that exists *and* names the sealed trait.
    ///
    /// # What this cannot do
    ///
    /// **It cannot tell whether a marker is true.** A lying `SEAL-EXACT` passes —
    /// measured, twice, and both times a `compile_fail` fixture caught the hole
    /// instead. That division of labour is the design: **the fixture pair is the
    /// enforcement; this check only forces a claim to be written down.** Earlier
    /// versions of the surrounding docs oversold it as the defence, which is worth
    /// not repeating.
    ///
    /// It is also a text scan, and three limits are known and deliberate:
    ///
    /// - a seal impl produced by a macro whose expansion never puts `impl` and the
    ///   seal name on one line is invisible. That matters from M2 on, when the
    ///   derive starts generating them.
    /// - `fixture:` is checked for existence and for naming the sealed trait, but
    ///   whether it *pins the difference* is semantic and cannot be checked here.
    ///   Measured: swapping a real-but-unrelated `Has` fixture in still passes. That
    ///   exact mistake was live — the depth `SEAL-DIFF` cited a fixture whose set is
    ///   well-formed, so the dropped bound was satisfied and the difference untested.
    ///   Review caught it, not this check.
    /// - `SEAL-EXACT` appearing in unrelated prose above an impl satisfies it.
    ///
    /// The floor (`seen >= 11`) is drift detection only, and it counts impl headers
    /// containing a seal name — not a semantic count of seal impls.
    #[test]
    fn every_seal_impl_should_declare_whether_it_mirrors_its_trait() {
        let seals = declared_seals();
        let mut problems: Vec<String> = Vec::new();
        let mut seen = 0usize;
        let files = source_files();
        assert!(
            files.len() >= 4,
            "only {} source files scanned",
            files.len()
        );

        for path in &files {
            let src = std::fs::read_to_string(path).expect("unreadable source");
            let name = path.display().to_string();

            // An alias hides the seal's name from the scan, so forbid aliasing
            // rather than trying to resolve it. Measured: `use ... as Ok` let an
            // unannotated wide-open seal impl through.
            for (i, line) in src.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("use ")
                    && t.contains(" as ")
                    && seals.iter().any(|s| t.contains(s.as_str()))
                {
                    problems.push(format!(
                        "{name}:{}: a seal must not be aliased — it hides the name from this check: {t}",
                        i + 1
                    ));
                }
            }

            for (line_no, item) in impl_headers(&src) {
                let Some(seal) = seals.iter().find(|s| item.contains(s.as_str())) else {
                    continue;
                };
                seen += 1;
                let mut block = String::new();
                for prev in src.lines().take(line_no).collect::<Vec<_>>().iter().rev() {
                    let prev = prev.trim_start();
                    if prev.starts_with("#[") {
                        continue;
                    }
                    if let Some(rest) = prev.strip_prefix("//") {
                        block.insert_str(0, rest);
                        continue;
                    }
                    break;
                }
                let exact = block.contains("SEAL-EXACT");
                let diff = block.contains("SEAL-DIFF");
                if exact == diff {
                    problems.push(format!(
                        "{name}:{}: seal impl needs exactly one of SEAL-EXACT / SEAL-DIFF: {}",
                        line_no + 1,
                        item.trim()
                    ));
                    continue;
                }
                if diff {
                    match block.split("fixture:").nth(1) {
                        None => problems.push(format!(
                            "{name}:{}: SEAL-DIFF without `fixture:` — a deliberate \
                             difference must be pinned by a test",
                            line_no + 1
                        )),
                        Some(rest) => {
                            let f = rest.split_whitespace().next().unwrap_or_default();
                            let p = std::path::Path::new("tests/ui/compile_fail").join(f);
                            match std::fs::read_to_string(&p) {
                                Err(_) => problems.push(format!(
                                    "{name}:{}: SEAL-DIFF cites `{f}`, which does not exist",
                                    line_no + 1
                                )),
                                // Existence is not enough: a fixture that never
                                // mentions the trait cannot be pinning its
                                // difference. Measured — swapping in an unrelated
                                // but real fixture passed.
                                Ok(text) => {
                                    let trait_name = seal.trim_start_matches("Sealed");
                                    if !text.contains(trait_name) {
                                        problems.push(format!(
                                            "{name}:{}: SEAL-DIFF cites `{f}`, which never mentions `{trait_name}`",
                                            line_no + 1
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(problems.is_empty(), "{}", problems.join("\n"));

        // Drift detection only. Additions are handled by the marker requirement,
        // which now covers every file — a count cannot do that, which is how the
        // `domain.rs` impl went unscanned.
        assert!(
            seen >= 11,
            "scanned only {seen} seal impls across {} files — the scan pattern has \
             drifted, so this check would pass by not looking",
            files.len()
        );
    }

    /// Which seals live in which module is a **security property**, not a layout
    /// choice, and nothing else tests it.
    ///
    /// `private` holds the structural seals: verum implements those itself over
    /// `()` and tuples, no derive ever will, so it stays `pub(crate)` permanently.
    /// `derive_facing` holds the ones a derive must satisfy, and M2 has no choice
    /// but to expose that module (`docs/rules/api-surface.md` §2). Moving a
    /// structural seal across that line would hand it to downstream code the
    /// moment M2 lands, reopening ledger paths 14a–14e.
    ///
    /// Measured: re-exporting `SealedAppend` / `SealedHas` / `SealedLookup`
    /// through `derive_facing` left the entire suite green, because today both
    /// modules are `pub(crate)` and the difference is invisible. Invisible now,
    /// load-bearing at M2 — which is exactly when nobody will be looking.
    ///
    /// **Deliberately single-file, unlike the other two guards.** `derive_facing`
    /// is one module declared here, and another file cannot add to its body, so
    /// widening the scan would not raise the detection power — `docs/rules/test.md`
    /// §9 rule 7 is about enumerating a set of files, and this check's subject is
    /// one module's contents.
    ///
    /// **That is only true of the file scope. The name list below is hardcoded,
    /// and that is a live hole — #65.** Moving an existing structural seal into
    /// `derive_facing` fails here; *adding* a new one does not, and M2 is exactly
    /// when `SealedEndpoint` / `SealedField` / `SealedCondition` arrive in this
    /// module. A `pub use super::private::*;` also passes. Both measured. The fix
    /// is to derive the list from `declared_seals()` minus an explicit allowlist,
    /// the way `seals_should_only_be_declared_through_the_macro` now derives its
    /// own — tracked separately because the defect is *what is named*, not *where
    /// it lives*.
    #[test]
    fn structural_seals_should_not_be_reachable_through_the_derive_facing_module() {
        let src = include_str!("sealed.rs");
        let (_, after) = src
            .split_once("pub(crate) mod derive_facing {")
            .expect("derive_facing module not found");
        // Only the module body. Without this bound the slice runs on into this very
        // test, whose own list of seal names would match and fail it — the check
        // would then be reporting on itself rather than on the module.
        let (derive_facing, _) = after
            .split_once("\n}\n")
            .expect("derive_facing module is not closed by a top-level brace");

        for structural in [
            "SealedConsList",
            "SealedIndex",
            "SealedHas",
            "SealedAppend",
            "SealedLookup",
        ] {
            assert!(
                !derive_facing.contains(structural),
                "`{structural}` is a structural seal and must not be declared in or \
                 re-exported through `derive_facing` — M2 exposes that module, which \
                 would reopen unverified-boundaries.md 14a–14e"
            );
        }

        // And the derive-facing set is exactly what M2 is allowed to expose.
        assert!(
            derive_facing.contains("SealedIncludes"),
            "`SealedIncludes` should be the derive-facing seal"
        );
    }

    // ----------------------------------------------------------------- the
    // guards' own tests. Scans run over the real tree can only show that today's
    // tree is clean — they cannot show the scan would notice if it were not.
    // Both directions are pinned against fixed input.
    //
    // The predicate is name-based, so a fixture is just a source line plus the
    // allow-list it is judged against. No `concat!` tricks are needed: an array
    // element trims to a leading `"`, which never matches `pub trait`. (An
    // earlier version used `concat!` and justified it with a hazard that was
    // measured not to exist for this fixture shape.)

    fn allow(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_trait_that_is_neither_sealed_nor_reexported_should_be_reported() {
        let src = ["pub trait MySeal {}"].join("\n");
        let found = handrolled_seals_in("src/domain.rs", &src, &allow(&["Includes"]));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("src/domain.rs:1:"), "{found:?}");
    }

    #[test]
    fn file_scope_should_not_be_a_free_pass() {
        // The predicate this replaced treated file scope as safe. Every module in
        // this crate is private (`lib.rs`: `mod domain; mod sealed; mod
        // typelevel;`), so file scope is *inside* a non-`pub` module and a trait
        // there is a usable seal — measured downstream, six ways.
        let src = ["pub trait E1Seal {}", "impl<T> E1Seal for T {}"].join("\n");
        assert_eq!(
            handrolled_seals_in("src/typelevel.rs", &src, &allow(&["ConsList"])).len(),
            1
        );
    }

    #[test]
    fn a_reexported_trait_should_not_be_reported() {
        // The six real sealed traits are public because `lib.rs` re-exports them,
        // which is the property that matters — not where they are declared.
        let src = [
            "pub trait ConsList: private::SealedConsList {}",
            "pub trait Has<T, Idx>: private::SealedHas<T, Idx> {}",
            "pub trait Includes<D>: derive_facing::SealedIncludes<D> {}",
        ]
        .join("\n");
        assert!(
            handrolled_seals_in(
                "src/typelevel.rs",
                &src,
                &allow(&["ConsList", "Has", "Includes"])
            )
            .is_empty()
        );
    }

    #[test]
    fn a_seal_declared_through_the_macro_should_not_be_reported() {
        let src = ["pub trait SealedConsList {}"].join("\n");
        assert!(handrolled_seals_in("src/sealed.rs", &src, &allow(&["SealedConsList"])).is_empty());
    }

    #[test]
    fn the_seal_macro_template_should_not_be_reported() {
        // `$name` is not an identifier, so `declared_pub_trait` returns None.
        let src = ["        pub trait $name $(<$($param),+>)? {}"].join("\n");
        assert!(handrolled_seals_in("src/sealed.rs", &src, &[]).is_empty());
    }

    #[test]
    fn a_trailing_dollar_name_comment_should_not_disable_the_check() {
        // The previous version skipped any line *containing* `$name`, so this
        // seven-character comment turned the guard off entirely (measured).
        let src = ["pub trait Forgeable {} // $name"].join("\n");
        assert_eq!(handrolled_seals_in("src/domain.rs", &src, &[]).len(), 1);
    }

    #[test]
    fn reexported_names_should_include_every_pub_use_target() {
        let names = reexported_names();
        for expected in ["Includes", "ConsList", "Has", "Append", "Lookup", "Index"] {
            assert!(
                names.iter().any(|n| n == expected),
                "`{expected}` is re-exported by lib.rs but not derived: {names:?}"
            );
        }
    }

    #[test]
    fn seal_names_should_be_found_in_any_file() {
        let src = ["seal! {", "    /// Doc.", "    SealedElsewhere", "}"].join("\n");
        assert_eq!(seal_names_in(&src), vec!["SealedElsewhere".to_string()]);
    }
}
