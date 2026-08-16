#!/usr/bin/env python3
"""Extract every Rust code block from `docs/` and report what it needs.

#43's requirement is that every block either compiles or is marked illustrative.
This is the extractor half: it finds the blocks, reads their fence tag, and
writes the checked ones out for compilation.

Tags follow rustdoc so GitHub renders them unchanged:

    ```rust                compile it, and it must succeed
    ```rust,compile_fail   compile it, and it must FAIL
    ```rust,ignore         do not compile it — a reason must appear in the
                           three lines above the fence

Anything else with a `rust`/`rs` tag is an error: an unmarked block that cannot
compile is exactly the state #43 exists to end.
"""

import argparse
import json
import pathlib
import re
import sys

FENCE = re.compile(r"^```(rust|rs)(?:,(.*))?\s*$")
# Resolved from this file, not from the working directory: run from the spike
# directory the relative form finds nothing, and "found nothing" reads exactly
# like "nothing is wrong". The FATAL below caught it; the fix is to not depend
# on where the script is invoked from.
DOCS = pathlib.Path(__file__).resolve().parents[2] / "docs"


def blocks(root: pathlib.Path):
    """Yield every fenced Rust block under `root`, with its tag and location."""
    for path in sorted(root.rglob("*.md")):
        lines = path.read_text().split("\n")
        i = 0
        while i < len(lines):
            m = FENCE.match(lines[i])
            if not m:
                i += 1
                continue
            j = i + 1
            while j < len(lines) and not lines[j].startswith("```"):
                j += 1
            if j >= len(lines):
                print(f"FATAL: unterminated fence at {path}:{i+1}", file=sys.stderr)
                sys.exit(1)
            yield {
                "file": str(path),
                "line": i + 1,
                "tag": (m.group(2) or "").strip(),
                "body": "\n".join(lines[i + 1 : j]),
                "lead": "\n".join(lines[max(0, i - 3) : i]),
            }
            i = j + 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=pathlib.Path, help="write checked blocks here")
    ap.add_argument("--json", action="store_true", help="emit the inventory as JSON")
    args = ap.parse_args()

    found = list(blocks(DOCS))
    if not found:
        # An empty scan reports success just as loudly as a clean one
        # (docs/rules/test.md §9-4). Refuse it.
        print("FATAL: no Rust blocks found under docs/ — the scan is broken", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(found, ensure_ascii=False, indent=1))
        return 0

    counts = {}
    for b in found:
        counts[b["tag"] or "(checked)"] = counts.get(b["tag"] or "(checked)", 0) + 1
    for tag, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        print(f"  {tag:<16} {n:>4}")
    print(f"  {'total':<16} {len(found):>4}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
