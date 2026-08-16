#!/usr/bin/env python3
"""Split fences that carry a ✅ form and a ❌ form in one block.

28 blocks in `docs/` put a correct example and an incorrect one inside a single
fence. No fence tag is true of such a block: tagging it `rust` asserts the ❌ half
compiles, and `compile_fail` asserts the ✅ half does not. So both halves go
unchecked, which is #43's thesis happening inside one code block.

Splitting them into consecutive fences keeps the side-by-side reading (two code
boxes, still adjacent, no prose inserted) and makes each half checkable:

    ```rust                the ✅ half — must compile
    ```rust,compile_fail   the ❌ half — must not

A half with no code of its own (a bare `// ❌ don't do this` annotation) is left
attached to its neighbour: there is nothing to check, and lifting a comment into
its own fence would be noise.
"""

import argparse
import collections
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from check import ROOT  # noqa: E402
from extract import blocks  # noqa: E402


def has_code(text: str) -> bool:
    return any(l.strip() and not l.strip().startswith("//") for l in text.split("\n"))


def segments(body: str) -> list[tuple[str, str]]:
    """Cut the body at ✅ / ❌ markers.

    Lines before the first marker belong to the first marked segment — they are
    usually a `use` line or a doc comment that both halves need.
    """
    out: list[tuple[str | None, list[str]]] = []
    cur: list[str] = []
    label: str | None = None
    for line in body.split("\n"):
        if "✅" in line or "❌" in line:
            if cur:
                out.append((label, cur))
            label = "ok" if "✅" in line else "bad"
            cur = [line]
        else:
            cur.append(line)
    if cur:
        out.append((label, cur))

    merged: list[tuple[str, str]] = []
    lead: list[str] = []
    for lab, lines in out:
        if lab is None:
            lead += lines
            continue
        merged.append((lab, "\n".join(lead + lines).strip("\n")))
        lead = []
    if lead and merged:  # trailing unlabelled lines stay with the last segment
        lab, text = merged[-1]
        merged[-1] = (lab, text + "\n" + "\n".join(lead).rstrip())
    return merged


def render(segs: list[tuple[str, str]]) -> list[str]:
    """One fence per segment that carries code; comment-only segments merge up."""
    fences: list[str] = []
    pending: list[str] = []
    for lab, text in segs:
        if not has_code(text):
            pending.append(text)
            continue
        body = "\n".join(pending + [text]) if pending else text
        pending = []
        tag = "rust,compile_fail" if lab == "bad" else "rust"
        fences += [f"```{tag}", body, "```", ""]
    if pending and fences:  # a trailing annotation with no code of its own
        fences.insert(len(fences) - 2, "\n".join(pending))
    if fences and fences[-1] == "":
        fences.pop()
    return fences


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    edits = collections.defaultdict(list)
    for b in blocks(ROOT / "docs"):
        if not ("✅" in b["body"] and "❌" in b["body"]):
            continue
        segs = segments(b["body"])
        if sum(1 for lab, t in segs if has_code(t)) < 2:
            continue  # only one checkable half — leave it to classify.py
        edits[b["file"]].append((b["line"], len(b["body"].split("\n")), render(segs)))

    if not edits:
        print("FATAL: no mixed blocks found — the scan is broken", file=sys.stderr)
        return 1

    total = 0
    for path, items in edits.items():
        p = pathlib.Path(path)
        lines = p.read_text().split("\n")
        for line, height, fences in sorted(items, reverse=True):
            i = line - 1
            assert lines[i].startswith("```"), f"{path}:{line} is not a fence"
            assert lines[i + height + 1].startswith("```"), f"{path}:{line} height mismatch"
            lines[i : i + height + 2] = fences
            total += 1
        if not args.dry_run:
            p.write_text("\n".join(lines))

    print(f"  split {total} blocks across {len(edits)} files"
          f"{'  (dry run)' if args.dry_run else ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
