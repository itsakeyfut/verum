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
    ledger's own emitted entries
  * the ledger's sample and `ai-context.md`'s emit the SAME set of `kind`s, and
    the ledger's counting rule enumerates exactly that set with the counts it
    states — the enumeration is verified from every end, so no copy can rot alone

    This is what "the join is verified from both ends" used to claim, and the
    code checked one direction: `voided_by` -> `kind`. #44 measured the cost of
    the gap — `forged_endpoint` and `forged_field` were added to the ledger's
    sample by #41 and to nothing else, the counting rule said "these twelve
    entries" while enumerating fourteen, and the ledger asserted that this file
    "makes the agreement mechanical rather than a promise".

    The OTHER join — every emitted `kind` being named by some `voided_by` — is
    deliberately NOT checked, because it does not hold and the fix is a design
    question, not a copy edit: `condition_body`, `row_scope`, `uncapped_read`,
    `forged_endpoint`, `forged_field` and `dyn_erasure` are named by no `voided_by`
    today, so a reader who stops at `enforcement` for the keys they void comes away
    believing the guarantee is unconditional. Which keys each of them voids is
    tracked separately — `dyn_erasure` voids the effects of a *service*, and there
    is no key for a service. Asserting it here with an exemption list of exactly
    those names would be an assertion that cannot fail, which is worse than none.
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
# The ledger's entry set is copied here, and the ledger says the two "must agree
# as a set". Nothing checked that until #44, and they had disagreed since #41.
AI_CONTEXT = DOCS / "specs" / "ai-context.md"

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
# `syntactically_present` is the token scan's result rather than a declaration, and
# carries its own `scope`. Named explicitly because a sample may show it on its own,
# with no enclosing `mutates` to inherit enforcement from.
#
# It was called `observed` until ADR-0014. This line is why the rename could not be
# half-done: with the old name here the checker demanded an `enforcement` object on
# the new key and reported it, which is how the incomplete rename surfaced.
CLAIMS_EXEMPT = {"syntactically_present"}


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


def emitted_entries(target):
    """`kind` -> `permanent` for every boundary entry `target` emits.

    Parsed rather than hard-coded so the two cannot drift: adding a kind to the
    checker without adding it to the file is not possible, and vice versa. The
    `permanent` flag comes along because the counting rule splits on it, and a
    split stated in prose beside a split stated in JSON is two copies.

    An entry whose `permanent` is missing or not a bool is reported by the caller
    rather than defaulted — defaulting would make a typo'd key read as
    `permanent: false`, which is the cheaper-looking half of the count.
    """
    entries = {}
    duplicates = []
    samples = 0
    for path, line, text in fences(target.parent):
        if path != target:
            continue
        doc = parse(text)
        if doc is None:
            continue

        # ONLY the emitted sample, not every fence in the file. Scanning all of
        # them and unioning the names is fail-open: #44's review deleted `row_scope`
        # from `ai-context.md`'s real sample, put the name in an illustrative
        # `"entries": [ ... ]` fragment elsewhere in the same file, and the
        # two-sample agreement below stayed green — which is the exact defect this
        # rule exists to catch, reproduced through the rule itself.
        found = []

        def sample(node):
            if isinstance(node, dict):
                if "completeness" in node and isinstance(node.get("entries"), list):
                    found.append(node["entries"])
                for v in node.values():
                    sample(v)
            elif isinstance(node, list):
                for v in node:
                    sample(v)

        sample(doc)
        # An illustrative fragment carries `[ ... ]`, which `parse` normalises to
        # the string "..." — no dict, so it contributes nothing and is not counted.
        for arr in found:
            if not any(isinstance(e, dict) for e in arr):
                continue
            samples += 1
            for e in arr:
                if not isinstance(e, dict) or not isinstance(e.get("kind"), str):
                    continue
                kind = e["kind"]
                if kind in entries:
                    # A dict silently collapses these, so a duplicated entry with a
                    # different flag used to disappear — while the counting rule
                    # says it counts ONE-TO-ONE with the entries emitted.
                    duplicates.append(f"{target.name}:{line}: entry {kind!r} is emitted twice")
                entries[kind] = e.get("permanent", "missing")
    return entries, duplicates, samples


def ledger_kinds():
    return set(emitted_entries(LEDGER)[0])


# `permanent 5 = `a` / `b` / ...` and `non-permanent 9 = ...`, from the ledger's
# own counting-rule note. The lookbehind matters: "non-permanent 9 =" contains
# "permanent 9 =" as a substring, and matching it would silently read the wrong
# list. Each segment ends at the next label or at the blank quote line that ends
# the paragraph.
COUNT_RULE = {
    "permanent": re.compile(
        r"(?<!non-)permanent\s+(\d+)\s*=(.*?)(?=non-permanent\s+\d+\s*=|\n>\s*\n|\Z)",
        re.S,
    ),
    "non-permanent": re.compile(
        r"non-permanent\s+(\d+)\s*=(.*?)(?=\n>\s*\n|\Z)", re.S
    ),
}
# `[a-z0-9_]+`, not `[a-z_]+`: a kind with a digit in it (`http2_smuggle`) could
# be added consistently in all three places and still fail here forever.
BACKTICKED = re.compile(r"`([a-z0-9_]+)`")

# The progress-metric block — the THIRD copy of the same two numbers, eleven lines
# above the note that claimed there was no third place for them to disagree from.
# #44 hand-edited it from `5 permanent + 9` to `6 permanent + 11` and nothing
# compared it to anything; `4 permanent + 99` was green.
PROGRESS_METRIC = re.compile(
    r"First PoC:\s+(\d+) permanent \+\s*(\d+) non-permanent.*?"
    r"Full PoC:\s+(\d+) permanent \+\s*(\d+) non-permanent",
    re.S,
)


def counting_rule():
    """The ledger's prose enumeration: label -> (stated count, [kind names]).

    Returns {} if the note cannot be found at all, which the caller treats as
    fatal — "the enumeration is absent" must not read like "the enumeration
    agrees".
    """
    text = LEDGER.read_text()
    out = {}
    for label, pattern in COUNT_RULE.items():
        m = pattern.search(text)
        if not m:
            continue
        out[label] = (int(m.group(1)), BACKTICKED.findall(m.group(2)))
    return out


def enumeration_problems():
    """The three copies of one entry set must agree: the ledger's sample,
    `ai-context.md`'s sample, and the ledger's counting rule.
    """
    problems = []
    ledger, ledger_dupes, ledger_samples = emitted_entries(LEDGER)
    context, context_dupes, context_samples = emitted_entries(AI_CONTEXT)
    problems += ledger_dupes + context_dupes

    if not ledger:
        return [f"FATAL: {LEDGER.name} emits no boundary entries — the scan is broken"]
    if not context:
        return [f"FATAL: {AI_CONTEXT.name} emits no boundary entries — the scan is broken"]
    for name, n in ((LEDGER.name, ledger_samples), (AI_CONTEXT.name, context_samples)):
        if n != 1:
            problems.append(
                f"{name}: {n} populated `unverified_boundaries.entries` samples — expected "
                f"exactly 1, because two would let one rot while the other satisfies the "
                f"agreement below"
            )

    for name, entries in ((LEDGER.name, ledger), (AI_CONTEXT.name, context)):
        for kind, perm in sorted(entries.items()):
            if not isinstance(perm, bool):
                problems.append(f"{name}: entry {kind!r} has permanent={perm!r}, expected a bool")

    # The flag, not just the name. Flipping `permanent` in one file only was green:
    # the set comparison below sees names, and the counting rule checks the ledger
    # side alone, so `ai-context.md` could rot by itself — which is the shape of
    # the very drift #44 was filed to fix.
    for kind in sorted(set(ledger) & set(context)):
        if ledger[kind] is not context[kind]:
            problems.append(
                f"entry {kind!r}: permanent={ledger[kind]!r} in {LEDGER.name} but "
                f"{context[kind]!r} in {AI_CONTEXT.name}"
            )

    only_ledger = sorted(set(ledger) - set(context))
    only_context = sorted(set(context) - set(ledger))
    if only_ledger:
        problems.append(
            f"{AI_CONTEXT.name}: missing the entries {only_ledger} that {LEDGER.name} emits "
            f"— the two samples must agree as a set"
        )
    if only_context:
        problems.append(
            f"{LEDGER.name}: missing the entries {only_context} that {AI_CONTEXT.name} emits "
            f"— the two samples must agree as a set"
        )

    rule = counting_rule()
    for label in ("permanent", "non-permanent"):
        if label not in rule:
            problems.append(
                f"{LEDGER.name}: the counting rule states no {label!r} enumeration — "
                f"a count nothing enumerates is the state the note exists to prevent"
            )
    if len(rule) < 2:
        return problems

    for label, (stated, names) in rule.items():
        if stated != len(names):
            problems.append(
                f"{LEDGER.name}: the counting rule says {label} {stated} and enumerates "
                f"{len(names)} ({names})"
            )
        dupes = sorted({n for n in names if names.count(n) > 1})
        if dupes:
            problems.append(f"{LEDGER.name}: the {label} enumeration repeats {dupes}")

    enumerated = set(rule["permanent"][1]) | set(rule["non-permanent"][1])
    both = sorted(set(rule["permanent"][1]) & set(rule["non-permanent"][1]))
    if both:
        problems.append(f"{LEDGER.name}: {both} is counted as permanent AND non-permanent")
    if enumerated != set(ledger):
        missing = sorted(set(ledger) - enumerated)
        extra = sorted(enumerated - set(ledger))
        problems.append(
            f"{LEDGER.name}: the counting rule enumerates a different set than the sample emits "
            f"— unenumerated {missing}, enumerated-but-not-emitted {extra}"
        )

    # The split itself, not just the total: prose saying `permanent` beside JSON
    # saying `permanent: false` is the disagreement the counting rule is for.
    for label, want in (("permanent", True), ("non-permanent", False)):
        for kind in rule[label][1]:
            if ledger.get(kind, "missing") is not want:
                problems.append(
                    f"{LEDGER.name}: the counting rule lists {kind!r} as {label}, "
                    f"but the sample emits permanent={ledger.get(kind, 'missing')!r}"
                )

    # The third copy.
    m = PROGRESS_METRIC.search(LEDGER.read_text())
    if not m:
        problems.append(
            f"{LEDGER.name}: the progress-metric block is missing or reworded — it is a "
            f"third copy of these counts and must be comparable"
        )
    else:
        fp_perm, fp_non, full_perm, full_non = (int(g) for g in m.groups())
        want_perm = sum(1 for v in ledger.values() if v is True)
        want_non = sum(1 for v in ledger.values() if v is False)
        if (fp_perm, fp_non) != (want_perm, want_non):
            problems.append(
                f"{LEDGER.name}: the progress metric says First PoC {fp_perm} permanent + "
                f"{fp_non} non-permanent, but the sample emits {want_perm} + {want_non}"
            )
        # `Full PoC` is First PoC minus the two paths its own parenthesis names.
        handled = {"middleware", "event_subscriber"}
        expect_full_non = want_non - len(handled & set(ledger))
        if (full_perm, full_non) != (want_perm, expect_full_non):
            problems.append(
                f"{LEDGER.name}: the progress metric says Full PoC {full_perm} + {full_non}, "
                f"but removing {sorted(handled)} from {want_non} non-permanent leaves "
                f"{expect_full_non} (permanent stays {want_perm})"
            )
    return problems


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

    problems = enumeration_problems()
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
    print(f"  ai-context     {len(emitted_entries(AI_CONTEXT)[0])} entries")
    if problems:
        print(f"  violations     {len(problems)}")
        for p in problems:
            print(f"    {p}", file=sys.stderr)
        return 1
    print("  violations     0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
