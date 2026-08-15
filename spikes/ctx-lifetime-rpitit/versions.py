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

deps = [
    p["rust_version"]
    for p in m["packages"]
    if p.get("rust_version") and p["name"] not in {"fw", "app"}
]
if not deps:
    print("FATAL: no dependency declared a rust-version — the tree is wrong", file=sys.stderr)
    sys.exit(1)

mx = max(deps, key=lambda v: [int(x) for x in v.split(".")])
print("  deps MSRV    {} (highest declared anywhere in the tree)".format(mx))
print("  packages     {}".format(len(m["packages"])))
