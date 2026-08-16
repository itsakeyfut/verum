#!/usr/bin/env python3
"""Write the fence tags `classify.py` proposes back into the documents.

Only the mechanically decidable categories are written. `REVIEW` blocks are left
untagged on purpose: an untagged block that fails is what `run.sh` reports, so
the failure count *is* the remaining work, and it goes down as blocks are fixed.

Reasons are written as a comment on the fence line itself
(``` rust,ignore // needs #[contract], which arrives in M2``) — rustdoc ignores
anything after the tag list and GitHub renders it unchanged, so the reason
travels with the block instead of living in a table someone has to find.
"""

import collections
import pathlib
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from check import ROOT  # noqa: E402
from classify import classify, compile_block  # noqa: E402
from extract import blocks  # noqa: E402

TAG = {
    "compile_fail": ("rust,compile_fail", None),
    "ignore:frag": ("rust,ignore", "fragment, not a complete item"),
    "ignore:macro": ("rust,ignore", "needs a macro that arrives in M2"),
    "ignore:external": ("rust,ignore", "needs a crate or a verum-private module this harness does not carry"),
    "text": ("text", None),
}


def main() -> int:
    deps = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path("target/debug/deps")
    dry = "--dry-run" in sys.argv
    tmp = pathlib.Path(tempfile.mkdtemp())

    found = list(blocks(ROOT / "docs"))
    if not found:
        print("FATAL: no Rust blocks found — the scan is broken", file=sys.stderr)
        return 1

    # Group by file and rewrite from the bottom up, so earlier line numbers stay
    # valid while later ones are edited.
    edits = collections.defaultdict(list)
    tally = collections.Counter()
    for n, b in enumerate(found):
        if b["tag"]:
            tally["already tagged"] += 1
            continue
        verdict = classify(b, compile_block(b["body"], deps, tmp, n))
        if verdict not in TAG:
            tally[verdict] += 1
            continue
        edits[b["file"]].append((b["line"], *TAG[verdict]))
        tally[verdict] += 1

    for path, items in edits.items():
        p = pathlib.Path(path)
        lines = p.read_text().split("\n")
        for line, tag, reason in sorted(items, reverse=True):
            i = line - 1
            assert lines[i].startswith("```"), f"{path}:{line} is not a fence"
            lines[i] = f"```{tag}" + (f"   // {reason}" if reason else "")
        if not dry:
            p.write_text("\n".join(lines))

    for k in sorted(tally):
        print(f"  {k:<24} {tally[k]:>4}")
    print(f"  {'files touched':<24} {len(edits):>4}{'  (dry run)' if dry else ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
