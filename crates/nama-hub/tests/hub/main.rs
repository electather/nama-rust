//! The Hub's integration tests, in one test program so a test build links the Hub once. Each
//! module was its own top-level test file; a new one goes here too (`crates/nama-hub/AGENTS.md`).
//! They run under nextest, which gives every test its own process (`README.md`).

#[path = "../support/mod.rs"]
mod support;

mod accounts;
mod config;
mod db;
mod deleting_accounts;
mod hub_info;
mod links;
mod passkeys;
mod server_addresses;
mod server_names;
mod sessions;
mod sharing;
mod titles;
mod track_preferences;
mod users_from_accounts;
mod viewing;
