#!/usr/bin/env python3
"""Check every published JSON sample against one AI Context schema.

#43 gave `docs/` a harness for Rust fences. The JSON fences had none — and the
AI Context schema is spread across six files that must agree key for key:

    ai-context.md  unverified-boundaries.md  mutation-contract.md
    read-contract.md  conditional-effects.md  effect-system.md

ADR-0008 changes that schema in eight ways. Six hand-maintained copies of one
schema with nothing checking them is the exact drift this project exists to
prevent, so the copies get a checker.

What is checked (ADR-0008 §Confirmation):

  * the string `type_checked` never appears — `docs/rules/test.md` has demanded
    this since it was written and nothing enforced it
  * every `enforcement` value is an object with exactly {level, scope, voided_by}
  * `level` and `scope` are drawn from their closed sets
  * `voided_by` is "not_applicable", or `kind` names that ALL appear in the
    ledger's own emitted entries — the join is verified from both ends, so
    neither side can rot alone
  * a key claiming a guarantee (`fields` / `domains` / `emits` / `calls`) carries
    an `enforcement` at all — the case that slipped through twice by hand
  * `escape_hatches`, `deferred` and `voided_by` are never `[]`
  * `unverified_boundaries` is an object carrying `completeness` and `entries`

Run it via `run.sh`. Exit 1 on any violation, and on finding no samples at all —
"scanned nothing" must not read like "nothing is wrong" (the lesson `extract.py`
records about its own DOCS path).
"""

import json
import pathlib
import re
import sys

# Resolved from this file, never the working directory — same reason extract.py
# does: run from elsewhere, a relative path finds nothing and reports success.
DOCS = pathlib.Path(__file__).resolve().parents[2] / "docs"
LEDGER = DOCS / "specs" / "unverified-boundaries.md"

ANY_FENCE = re.compile(r"^(`{3,})")
JSON_FENCE = re.compile(r"^```json\s*$")

LEVELS = {"upper_bound_checked", "intent_only", "metadata_only", "none"}
SCOPES = {"handle_via_ctx", "declaration_only", "none"}
ENFORCEMENT_KEYS = {"level", "scope", "voided_by"}
COMPLETENESS = {"best_effort", "exhaustive"}

# Keys that must never be an empty list: each would read as "nothing here",
# which is the one claim none of them can support.
NEVER_EMPTY = ("voided_by", "escape_hatches", "deferred")

# An object listing any of these is claiming a guarantee about what the endpoint
# does, so it must say how far that claim reaches. Checking the *shape* of an
# `enforcement` that is present does not catch the case that actually happened
# twice while writing #38: a guarantee-claiming key with no `enforcement` at all
# (`unconditional.emits` / `calls`, and the `conditional[]` entry in
# `conditional-effects.md`). Both were found by hand; this rule finds them.
CLAIMS = {"fields", "domains", "emits", "calls"}
# `observed` is the token scan's result rather than a declaration, and carries its
# own `scope`. Named explicitly because a sample may show it on its own, with no
# enclosing `mutates` to inherit enforcement from.
CLAIMS_EXEMPT = {"observed"}


def fences(root):
    """Yield (path, line, text) for every ```json block under `root`.

    Backtick length is tracked for the same reason extract.py tracks it: a block
    that *shows* fence syntax opens with ```` and contains ``` lines that are
    content. ADR-0003 is such a document.
    """
    for path in sorted(root.rglob("*.md")):
        lines = path.read_text().split("\n")
        i = 0
        while i < len(lines):
            if JSON_FENCE.match(lines[i]):
                j = i + 1
                while j < len(lines) and not lines[j].startswith("```"):
                    j += 1
                if j >= len(lines):
                    print(f"FATAL: unterminated json fence at {path}:{i+1}", file=sys.stderr)
                    sys.exit(1)
                yield path, i + 1, "\n".join(lines[i + 1 : j])
                i = j + 1
                continue
            outer = ANY_FENCE.match(lines[i])
            if outer:
                n = len(outer.group(1))
                j = i + 1
                while j < len(lines) and not re.match(r"^`{%d,}" % n, lines[j]):
                    j += 1
                i = j + 1
                continue
            i += 1


# A bare `...` stands for elided content in an illustrative snippet — `[ ... ]`,
# `"fields": [...]`. It is not JSON, so it is normalised to the string "...",
# which the key checks then treat as an elision rather than as a real value.
ELISION = re.compile(r"(?<!\")\.\.\.(?!\")")


def parse(text):
    """Parse a sample, tolerating fragments and elisions.

    Several samples are object *fragments* — `"mutates": { ... }` — because they
    illustrate one key. Wrapping in braces is tried after the plain form so a
    complete document is never misread as a fragment.
    """
    elided = ELISION.sub('"..."', text)
    for candidate in (text, elided, "{" + text + "}", "{" + elided + "}"):
        try:
            return json.loads(candidate)
        except json.JSONDecodeError:
            continue
    return None


def walk(node, fn):
    if isinstance(node, dict):
        for k, v in node.items():
            fn(k, v)
            walk(v, fn)
    elif isinstance(node, list):
        for v in node:
            walk(v, fn)


def ledger_kinds():
    """The `kind` values the ledger actually emits, read from its own sample.

    Parsed rather than hard-coded so the two cannot drift: adding a kind to the
    checker without adding it to the ledger is not possible, and vice versa.
    """
    kinds = set()
    for path, _, text in fences(LEDGER.parent):
        if path != LEDGER:
            continue
        doc = parse(text)
        if doc is None:
            continue
        walk(doc, lambda k, v: kinds.add(v) if k == "kind" and isinstance(v, str) else None)
    return kinds


def claims_without_enforcement(node, where, problems, key=None, covered=False):
    """Report a guarantee-claiming object that carries no `enforcement`.

    `covered` is the ancestry: a nested breakdown such as `mutates.conditional`
    is qualified by the `enforcement` on `mutates`, and must not be asked to
    repeat it. Without that, the rule fires on every per-condition summary —
    which is what the first version did.
    """
    if isinstance(node, dict):
        if (
            (CLAIMS & set(node))
            and "enforcement" not in node
            and not covered
            and key not in CLAIMS_EXEMPT
        ):
            claimed = sorted(CLAIMS & set(node))
            problems.append(
                f"{where}: `{key}` claims {claimed} with no `enforcement` — every key "
                f"that claims a guarantee carries level/scope/voided_by (ADR-0008)"
            )
        here = covered or "enforcement" in node
        for k, v in node.items():
            claims_without_enforcement(v, where, problems, k, here)
    elif isinstance(node, list):
        for v in node:
            claims_without_enforcement(v, where, problems, key, covered)


def main():
    kinds = ledger_kinds()
    if not kinds:
        print(f"FATAL: no `kind` values found in {LEDGER} — the scan is broken", file=sys.stderr)
        return 1

    problems = []
    samples = 0

    for path, line, text in fences(DOCS):
        samples += 1
        where = f"{path.relative_to(DOCS.parent)}:{line}"

        if "type_checked" in text:
            problems.append(f"{where}: `type_checked` appears — the check is upper-bound only")

        doc = parse(text)
        if doc is None:
            problems.append(f"{where}: not parseable as JSON, whole or as an object fragment")
            continue

        def inspect(key, value, where=where):
            if key == "enforcement":
                if not isinstance(value, dict):
                    problems.append(f"{where}: `enforcement` is {type(value).__name__}, expected an object (ADR-0008)")
                    return
                if set(value) != ENFORCEMENT_KEYS:
                    problems.append(f"{where}: `enforcement` keys {sorted(value)} != {sorted(ENFORCEMENT_KEYS)}")
                    return
                if value["level"] not in LEVELS:
                    problems.append(f"{where}: level {value['level']!r} outside {sorted(LEVELS)}")
                if value["scope"] not in SCOPES:
                    problems.append(f"{where}: scope {value['scope']!r} outside {sorted(SCOPES)}")

            if key == "voided_by":
                if value == "not_applicable":
                    return
                if not isinstance(value, list):
                    problems.append(f"{where}: `voided_by` must be a list or \"not_applicable\", got {value!r}")
                    return
                for name in value:
                    if name == "...":
                        continue  # an elision in an illustrative snippet
                    if name not in kinds:
                        problems.append(
                            f"{where}: voided_by names {name!r}, which is not a kind the ledger emits"
                        )

            if key == "unverified_boundaries":
                if not isinstance(value, dict):
                    problems.append(f"{where}: `unverified_boundaries` must be an object with completeness + entries")
                    return
                if set(value) != {"completeness", "entries"}:
                    problems.append(f"{where}: `unverified_boundaries` keys {sorted(value)} != ['completeness', 'entries']")
                    return
                if value["completeness"] not in COMPLETENESS:
                    problems.append(f"{where}: completeness {value['completeness']!r} outside {sorted(COMPLETENESS)}")

            if key in NEVER_EMPTY and value == []:
                problems.append(f"{where}: `{key}` is [] — emit \"unknown\" / \"not_applicable\" instead")

        walk(doc, inspect)
        claims_without_enforcement(doc, where, problems)

    if samples == 0:
        print(f"FATAL: no json fences found under {DOCS} — the scan is broken", file=sys.stderr)
        return 1

    print(f"  json samples   {samples}")
    print(f"  ledger kinds   {len(kinds)}")
    if problems:
        print(f"  violations     {len(problems)}")
        for p in problems:
            print(f"    {p}", file=sys.stderr)
        return 1
    print("  violations     0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
