#!/usr/bin/env python3
"""Create the SQLite file that sqlx's compile-time-checked macros need.

`sqlx::query_as!` verifies the SQL against a real database at *compile* time, so
without this there is no way to answer the issue's question about the macro form
as distinct from the runtime-checked `query_as::<_, T>` function.

Python's stdlib `sqlite3` is used on purpose: the `sqlite3` CLI is not present on
this machine, and requiring `sqlx-cli` (a from-source install) to re-check a
verdict is friction that would stop anyone re-checking it.
"""

import pathlib
import sqlite3
import sys

DB = pathlib.Path(__file__).with_name("spike.db")

SCHEMA = """
CREATE TABLE users (
    id    INTEGER PRIMARY KEY NOT NULL,
    name  TEXT NOT NULL,
    email TEXT NOT NULL
);
"""

if DB.exists():
    DB.unlink()

conn = sqlite3.connect(DB)
conn.executescript(SCHEMA)
conn.commit()

# Assert the work happened rather than that no error surfaced. A `query_as!`
# against an absent table fails at compile time with a message that reads like a
# spike result, so a silently empty database would be misread as a finding.
cols = [r[1] for r in conn.execute("PRAGMA table_info(users)")]
conn.close()

if cols != ["id", "name", "email"]:
    print(f"FATAL: users table came out as {cols}", file=sys.stderr)
    sys.exit(1)

print(f"ok: {DB} created with users({', '.join(cols)})")
