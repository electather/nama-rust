//! How the Hub crate's files are laid out. The rules are under "Structure" in
//! `crates/nama-hub/AGENTS.md`; `nama-structure` checks them against the tables here.

use nama_structure::{Allowed, Confined, Exception, MustFind, Structure};

#[test]
fn the_hub_keeps_its_structure() {
    Structure {
        root: env!("CARGO_MANIFEST_DIR"),
        ceilings: CEILINGS,
        owned_tables: OWNED_TABLES,
        rows: ROWS,
        // Each area arrives with the first ticket that needs it (ADR 0099).
        rows_name_areas_to_come: true,
        startup: STARTUP,
        allowed_upward: ALLOWED_UPWARD,
        surface_exceptions: SURFACE_EXCEPTIONS,
        confined_crates: CONFINED_CRATES,
        must_find: MustFind {
            nested_folder: "tests/support",
            area_module: None,
            crate_paths: 1,
            named_paths: 5,
        },
    }
    .check();
}

/// Today's files over 800 lines, sorted by path, each with the ceiling it stays at or under.
const CEILINGS: &[(&str, usize)] = &[];

/// The tables each area owns, each with its owner's path without `.rs`.
const OWNED_TABLES: &[(&str, &[&str])] = &[];

/// The Hub's modules in rows of who may call whom, highest row first (ADR 0099).
const ROWS: &[&[&str]] = &[
    &["api"],
    &["recommendations", "diagnostics", "discovery"],
    &["sync", "viewing"],
    &["links"],
    &["accounts", "titles"],
    &["signing", "mail", "web", "db", "config"],
];

/// The modules that start the Hub, which call any row.
const STARTUP: &[&str] = &["lib", "main"];

/// The calls against `ROWS` that are allowed anyway: who makes them, the leading segments of
/// the `crate::` path they name, and why. An entry no import uses fails.
const ALLOWED_UPWARD: &[Allowed] = &[];

/// The modules neighbours reach inside an area anyway, each with why re-exporting it would be
/// wrong. An entry no import uses fails.
const SURFACE_EXCEPTIONS: &[Exception] = &[];

/// The crates only one area may name.
const CONFINED_CRATES: &[Confined] = &[(
    "nama_hub_proto",
    "api",
    "ADR 0099: areas take and return their own types, and api converts",
)];
