//! The Nama Hub. Only a stub for now: the Hub is built in a later milestone.

use std::process::ExitCode;

#[expect(
    clippy::print_stderr,
    reason = "stub binary with no logging set up yet"
)]
fn main() -> ExitCode {
    eprintln!("nama-hub is not implemented yet");
    ExitCode::FAILURE
}
