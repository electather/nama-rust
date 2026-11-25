//! The helpers the Hub's integration tests share, which the `hub` test program declares once. The
//! folder stays at `tests/support/`, where the structure test's walk looks for a nested folder.

pub(crate) mod hub;
pub(crate) mod linking;
pub(crate) mod mail;
pub(crate) mod passkey;
pub(crate) mod refused;
