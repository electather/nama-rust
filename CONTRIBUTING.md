# Contributing to Nama

[`README.md`](README.md) says what each folder is. This page says where the rules are and how a change lands.

## Where the rules live

- **`AGENTS.md` files** hold the rules code must follow, each rule once, in the file nearest the code it governs: [the workspace's](AGENTS.md), then each package's (such as [`crates/nama-server/AGENTS.md`](crates/nama-server/AGENTS.md) or [`web/AGENTS.md`](web/AGENTS.md)), then each Server area's (such as [`crates/nama-server/src/scanner/AGENTS.md`](crates/nama-server/src/scanner/AGENTS.md)). Read every one on the way from the root to the files you change.
- **[`docs/adr/`](docs/adr)** records the decisions behind the design and the options they rejected. A change that reverses one adds a new ADR.
- **[`GLOSSARY.md`](GLOSSARY.md)** defines the words used in code, docs and the UI, and the words to avoid.

## The Server's layout

The Server ([`crates/nama-server`](crates/nama-server)) is one crate grouped into areas, one folder each beside its `.rs` file (`scanner.rs` with `scanner/`), plus a few single files. Areas and single files sit in fixed rows, and a module calls only a lower row or a module to its right in its own row. The rows live once, as `ROWS` in [`crates/nama-server/tests/structure.rs`](crates/nama-server/tests/structure.rs), and [ADR 0051](docs/adr/0051-the-servers-areas-call-down-fixed-rows.md) explains them. That test also holds file sizes and table owners in place.

## Checks

Run what CI runs, listed under "Checks" in [`README.md`](README.md#checks), and the web app's checks under "Checks" in [`web/README.md`](web/README.md#checks) when you change `web/`. CI fails on any warning.

## Structural changes are move-only

A change that moves or splits code covers one area and changes no logic, so a reviewer reading `git diff -M --color-moved` sees only moves. Its rules move with it, into the nearest `AGENTS.md`. A fix or behaviour change to moved code lands as its own change after the move.

## Commits

Commit messages are 10 words or fewer.

## Writing a Plugin

A Plugin builds against the Plugin contract through `nama-pdk` alone. [`crates/nama-pdk/README.md`](crates/nama-pdk/README.md) walks through writing one, and [`plugins/template/`](plugins/template) is the Plugin to copy. A Plugin in this repository also follows [`plugins/AGENTS.md`](plugins/AGENTS.md).
