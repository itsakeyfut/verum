#!/usr/bin/env python3
"""Propose a fence tag for every Rust block in `docs/`.

Most categories are decidable mechanically. The one that is not is kept
separate on purpose: a block that fails to compile and is *not* a failure
example, a fragment, macro-dependent, or reaching for a crate this harness does
not carry is a genuine defect in the documentation, which is what #43 is for.

  compile_fail    the block itself presents the code as something that must NOT
                  compile (❌ / コンパイルエラー / 通らない / ...)
  ignore:macro    needs `#[contract]` / `#[endpoint]` / `#[derive(Domain)]`,
                  which do not exist until M2
  ignore:frag     does not parse as a set of items — a bare `where` clause, a
                  signature without a body, statements outside a function
  ignore:external needs a crate this harness deliberately does not carry
                  (`thiserror`, `sqlx`, `quote`, …) or a `verum`-private module
  text            no code at all — prose inside a Rust fence
  REVIEW          none of the above. Read it.
"""

import collections
import json
import pathlib
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from check import PRELUDE, ROOT, rustc_args  # noqa: E402
from extract import blocks  # noqa: E402

NEGATIVE = ("❌", "コンパイルエラー", "型エラー", "通らない", "できない", "不可",
            "禁止", "やってはいけない", "compile_fail", "error[E", "拒否",
            "エラーになる", "落ちる", "弾かれる", "書けない")
MACRO = re.compile(r"#\[(contract|endpoint|derive\s*\([^)]*\b(Domain|Request|View|Endpoint|Repository)\b"
                   r"|get|post|put|delete|patch|escape_hatch|service|proc_macro)")
ITEM = re.compile(r"^\s*(//|/\*|#!|#\[|pub |fn |struct |enum |trait |impl |mod |use "
                  r"|const |static |type |macro_rules|extern |unsafe )")

# rustc emits syntax errors, and a few resolution failures, with no error code.
# That absence is the signal: a block that does not parse is a fragment, not a
# documentation defect. Reading only the block's first line — which the first
# version did — scored `pub fn foo(..);` and `impl X { fn y(..); }` as complete
# items and sent 26 fragments to REVIEW as if the specs were wrong.
# Named, not pattern-matched: "the block reached for something this harness does
# not carry" must be a decision recorded here, not a guess from an error string.
# Anything reaching for a name NOT on this list stays in REVIEW.
ABSENT = (
    # third-party crates the docs legitimately show
    "thiserror", "tokio", "sqlx", "quote", "syn", "proc_macro2", "uuid", "http",
    "tracing", "trybuild", "serde", "serde_json", "instant", "criterion",
    "verum_macros", "axum", "hyper", "tower", "insta",
    # `verum`'s own crate-private modules — unreachable from any downstream
    # crate by construction, so a block showing one can never compile here
    "private", "derive_facing", "__private",
)

# `crates/verum`'s own impls, quoted in the docs. `impl Has<H, There<I>> for
# (X, T)` and `impl Ctx<'req, E> { .. }` are legal in the crate that owns the
# trait or the type, and nowhere else — so compiling them as a downstream crate
# is the wrong question, not a documentation defect.
INTERNAL = (
    "only traits defined in the current crate can be implemented for arbitrary types",
    "must be used as the type parameter for some local type",
    "cannot define inherent `impl` for a type outside of the crate",
)

# A block that shows a declaration with its body elided — `{ /* ... */ }`, `{ }`
# — is illustrating a shape, not a compilable item. rustc then complains about
# what the missing body would have provided.
ELIDED = (
    "not all trait items implemented",
    "is never used",
)

# rustc reports these without a usable code, or with a code borrowed from a
# later error. Either way the block did not parse, which makes it a fragment.
PARSE = (
    "free function without a body", "non-item in item list", "expected item, found",
    "associated function in `impl` without body", "unexpected end of macro invocation",
    "expected one of", "unexpected token", "unknown start of token",
    "expected identifier, found",
    "missing `fn` or `struct`", "expected expression",
)


def code_lines(body: str) -> list[str]:
    """Lines that are neither blank nor a `//` comment."""
    return [line for line in body.split("\n")
            if line.strip() and not line.strip().startswith("//")]


def compile_block(body: str, deps: pathlib.Path, tmp: pathlib.Path, n: int) -> dict:
    src = tmp / f"b{n:04d}.rs"
    src.write_text(PRELUDE + body + "\n")
    r = subprocess.run(rustc_args(tmp, deps, src), capture_output=True, text=True)
    errs = []
    for line in r.stderr.splitlines():
        if not line.startswith("{"):
            continue
        d = json.loads(line)
        if d.get("level") == "error":
            errs.append(d)
    return {
        "fails": r.returncode != 0,
        "codes": sorted({(e.get("code") or {}).get("code") for e in errs} - {None}),
        "messages": [e.get("message", "") for e in errs],
    }


def _both_halves_have_code(body: str) -> bool:
    """True when a ✅/❌ block has code on both sides.

    Only then is it splittable. A block whose ❌ side is a bare annotation
    (`// ❌ don't do this`, no code) has nothing to split off, so it goes through
    normal classification rather than sitting in REVIEW forever.
    """
    marked, cur, label = [], [], None
    for line in body.split("\n"):
        if "✅" in line or "❌" in line:
            if cur and label:
                marked.append((label, cur))
            label = "ok" if "✅" in line else "bad"
            cur = [line]
        else:
            cur.append(line)
    if cur and label:
        marked.append((label, cur))
    sides = {lab for lab, lines in marked
             if any(l.strip() and not l.strip().startswith("//") for l in lines)}
    return len(sides) == 2


def classify(b: dict, res: dict) -> str:
    body = b["body"]

    # A block with no code at all is prose in a Rust fence. It compiles
    # trivially, so tagging it `rust` says "checked" while checking nothing.
    if not code_lines(body):
        return "text"

    if MACRO.search(body):
        return "ignore:macro"

    # The block's OWN text only. The first version also read the three lines of
    # prose above the fence, and every one of the seven hits that produced was a
    # false positive — the prose was about something else.
    # A block carrying both ✅ and ❌ is showing a good form and a bad one side by
    # side. No single fence tag is true of it, and tagging it `compile_fail`
    # asserts the ✅ half fails. Split it or make it prose — either way a person
    # decides. (Measured: three such blocks were mis-tagged on the first sweep,
    # including the `Handler` declaration in `docs/rules/rust.md`.)
    if "✅" in body and "❌" in body and _both_halves_have_code(body):
        return "REVIEW:mixed"

    if any(t in body for t in NEGATIVE):
        if res["fails"]:
            return "compile_fail"
        # It compiles. If the ❌ side carries no code, the marker is an
        # annotation about an alternative, not a claim that this block fails —
        # `docs/rules/perf.md:41` and `docs/rules/rust.md:76` are both that shape.
        if "❌" in body and not _both_halves_have_code(body):
            return "ok"
        return "REVIEW:negative-but-compiles"

    if not ITEM.match(body.lstrip("\n")):
        return "ignore:frag"

    if not res["fails"]:
        return "ok"

    msgs = res["messages"]
    if any(t in m for m in msgs for t in PARSE) or "(...)" in body or "(..)" in body:
        return "ignore:frag"
    if any(t in m for m in msgs for t in INTERNAL):
        return "ignore:internal"
    if any(t in m for m in msgs for t in ELIDED) and ("/* ... */" in body or "{ }" in body
                                                     or "{}" in body or "…" in body):
        return "ignore:elided"
    if any(f"`{name}`" in m for m in msgs for name in ABSENT):
        return "ignore:external"

    return "REVIEW"


def main() -> int:
    deps = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path("target/debug/deps")
    tmp = pathlib.Path(tempfile.mkdtemp())
    found = list(blocks(ROOT / "docs"))
    if not found:
        print("FATAL: no Rust blocks found — the scan is broken", file=sys.stderr)
        return 1

    tally = collections.Counter()
    review = []
    for n, b in enumerate(found):
        if b["tag"]:
            tally[f"(tagged) {b['tag'].split()[0]}"] += 1
            continue
        res = compile_block(b["body"], deps, tmp, n)
        verdict = classify(b, res)
        tally[verdict] += 1
        if verdict.startswith("REVIEW"):
            review.append((b, verdict, res))

    for k in sorted(tally):
        print(f"  {k:<34} {tally[k]:>4}")
    print(f"  {'total':<34} {len(found):>4}")

    if review:
        print("\n  人間が読む必要があるもの:")
        for b, v, res in review:
            rel = pathlib.Path(b["file"]).relative_to(ROOT)
            codes = ",".join(res["codes"]) or "-"
            print(f"    {codes:<8} {rel}:{b['line']:<5} {res['messages'][0][:52] if res['messages'] else ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
