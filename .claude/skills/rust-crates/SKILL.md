---
name: rust-crates
description: Writing, checking and reviewing Rust in this repo. Use when a change touches a `.rs` file or a `Cargo.toml`.
---

# Rust crates

## Write

Load the `rust-best-practices` skill and write to it. Read a chapter in its `references/` only when the change touches that topic. The `AGENTS.md` and `README.md` of each crate you touch hold that crate's rules and workflow, such as proto codegen and sqlx query data.

## Check

Run `cargo fmt --all`, then every command in the root `README.md` "Checks", until all pass. Where a tool isn't installed (the sandcastle image has no `cargo nextest`, `cargo machete` or `cargo deny`), run `cargo test --workspace` in place of nextest and leave the other two to CI.

## Review

Hard findings, on top of `AGENTS.md`: `unwrap()` outside tests, and `unsafe` the code could do without.

If the diff touches async code (`async fn`, `tokio::`, `.await`, spawned tasks, channels, locks), run the `tokio-async-code-review` skill over those files and verify its findings with `review-verification-protocol`. Its Critical and Major findings are hard findings.
