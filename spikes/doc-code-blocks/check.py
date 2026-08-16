#!/usr/bin/env python3
"""Compile every checked Rust block in `docs/` and report the outcome.

Each block is compiled **on its own** with `rustc`, not as part of one crate: a
single parse error in one block would otherwise abort the whole run and every
other block's result would be unknown while the exit status still said "failed".

The prelude is a glob import of the stub. A block that defines its own `User`
shadows the glob, so blocks that are self-contained stay self-contained.
"""

import argparse
import collections
import json
import pathlib
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from extract import blocks  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
PRELUDE = """#![allow(unused, non_camel_case_types, incomplete_features, clippy::all)]
extern crate stub;
extern crate verum;
use stub::*;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
"""


def externs(deps: pathlib.Path) -> list[str]:
    """Resolve each dependency to ONE rlib.

    `-L` alone gives `E0464: multiple candidates` as soon as both `cargo check`
    and `cargo build` have run, because each leaves its own `.rmeta`. Naming the
    rlib explicitly is what makes the run reproducible regardless of what was
    built before.
    """
    out = []
    for name in ("stub", "verum"):
        libs = sorted(deps.glob(f"lib{name}-*.rlib"))
        if len(libs) != 1:
            print(f"FATAL: expected exactly one lib{name}-*.rlib, found {len(libs)}",
                  file=sys.stderr)
            sys.exit(1)
        out += ["--extern", f"{name}={libs[0]}"]
    return out


def rustc_args(tmp: pathlib.Path, deps: pathlib.Path, src: pathlib.Path):
    # `--test`, not `--crate-type lib`: without it a `#[test]` function is
    # stripped before type checking, so its body is never compiled and the block
    # is scored as passing. Measured — `docs/rules/test.md` §9-4 is exactly this
    # class, and this harness had it.
    return [
        "rustc", "+1.85.0", "--edition", "2024", "--test",
        "--error-format", "json", "--emit", "metadata",
        "-L", str(deps), *externs(deps),
        "-o", str(tmp / (src.stem + ".rmeta")), str(src),
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--deps", type=pathlib.Path, required=True)
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    found = [b for b in blocks(ROOT / "docs")]
    if not found:
        print("FATAL: no Rust blocks found — the scan is broken", file=sys.stderr)
        return 1

    tmp = pathlib.Path(tempfile.mkdtemp())
    tally = collections.Counter()
    failures = []

    for n, b in enumerate(found):
        tag = b["tag"]
        if "ignore" in tag:
            tally["ignore"] += 1
            continue
        expect_fail = "compile_fail" in tag
        src = tmp / f"b{n:04d}.rs"
        src.write_text(PRELUDE + b["body"] + "\n")
        r = subprocess.run(rustc_args(tmp, args.deps, src), capture_output=True, text=True)
        got_fail = r.returncode != 0
        key = "compile_fail" if expect_fail else "checked"
        if got_fail == expect_fail:
            tally[key + ":ok"] += 1
        else:
            tally[key + ":BAD"] += 1
            codes = sorted({
                (json.loads(l).get("code") or {}).get("code")
                for l in r.stderr.splitlines()
                if l.startswith("{") and json.loads(l).get("level") == "error"
            } - {None})
            failures.append((b, codes))

    for k in sorted(tally):
        print(f"  {k:<18} {tally[k]:>4}")
    print(f"  {'total':<18} {len(found):>4}")

    if failures and args.verbose:
        print("\n  失敗したブロック:")
        for b, codes in failures:
            print(f"    {b['file']}:{b['line']:<5} {','.join(codes) or '(no code)'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
