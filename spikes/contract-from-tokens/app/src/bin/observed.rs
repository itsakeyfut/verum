//! Prints every `__VERUM_OBSERVED_*` const, one JSON per line. `run.sh` asserts these.
//! The macro's output is the verdict, so it has to be observed, not inferred.
// `println!` is banned in the library (clippy.toml — Verum uses tracing). This is
// the harness's observation channel, not library code: the macro's output IS the
// verdict, so it has to reach `run.sh` on stdout.
#![allow(clippy::disallowed_macros)]

use app::*;

fn main() {
    for (name, json) in [
        ("UpdateUser", __VERUM_OBSERVED_UPDATE_USER),
        ("SneakyControl", __VERUM_OBSERVED_SNEAKY_CONTROL),
        ("EscapeHatch", __VERUM_OBSERVED_ESCAPE_HATCH),
        ("Aliased", __VERUM_OBSERVED_ALIASED),
        ("ViaHelper", __VERUM_OBSERVED_VIA_HELPER),
        ("Noop", __VERUM_OBSERVED_NOOP),
        ("RenamedCtx", __VERUM_OBSERVED_RENAMED_CTX),
        ("CfgGated", __VERUM_OBSERVED_CFG_GATED),
        ("NestedWhen", __VERUM_OBSERVED_NESTED_WHEN),
        ("NestedFnHelper", __VERUM_OBSERVED_NESTED_FN_HELPER),
        ("Ufcs", __VERUM_OBSERVED_UFCS),
        ("MacroExpanded", __VERUM_OBSERVED_MACRO_EXPANDED),
        ("DeadCode", __VERUM_OBSERVED_DEAD_CODE),
    ] {
        println!("{name}\t{json}");
    }
}
