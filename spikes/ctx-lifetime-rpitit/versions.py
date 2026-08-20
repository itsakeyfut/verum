#!/usr/bin/env python3
"""Print the dependency versions this run measured.

A separate file rather than a `python3 -c` heredoc inside run.sh: the quoting
required to nest an f-string containing double quotes inside a single-quoted
shell argument is a source of syntax errors that surface only at run time, and
this script is the *first* thing run.sh does — a break here reads like a
resolution failure rather than a typo.
"""

import json
import sys

m = json.load(sys.stdin)
want = {"tokio", "hyper", "hyper-util", "http-body-util"}

for p in sorted(m["packages"], key=lambda p: p["name"]):
    if p["name"] in want:
        print("  {:<15}{}".format(p["name"], p["version"]))

found = {p["name"] for p in m["packages"]} & want
if found != want:
    print("FATAL: missing from the tree: " + str(sorted(want - found)), file=sys.stderr)
    sys.exit(1)

# Derived from `workspace_members`, not hand-listed. The list was `{"fw", "app"}`
# and did not follow `downstream` when #44 added it, so the printed "highest
# declared anywhere" became the spike's own member value (1.85) instead of the
# dependency maximum (1.71) — a number that then could not move. The two path
# dependencies on the production crates are excluded for the same reason: they are
# ours, not the tree's. RK-016 rule (b).
member_names = {
    p["name"] for p in m["packages"] if p["id"] in set(m.get("workspace_members", []))
}
ours = member_names | {"verum", "verum-macros"}
deps = [
    p["rust_version"]
    for p in m["packages"]
    if p.get("rust_version") and p["name"] not in ours
]
if not deps:
    print("FATAL: no dependency declared a rust-version — the tree is wrong", file=sys.stderr)
    sys.exit(1)

mx = max(deps, key=lambda v: [int(x) for x in v.split(".")])
print("  deps MSRV    {} (highest declared by a DEPENDENCY — our own members are excluded, derived from workspace_members)".format(mx))
print("  packages     {}".format(len(m["packages"])))
