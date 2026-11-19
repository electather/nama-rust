//! ffmpeg's process as a [`Run`].

use std::process::Output;

use tokio::process::Child;

use super::run::{Outcome, Run};

/// `child`, an ffmpeg started with its stderr piped and to be killed on drop, as a run, which so
/// stops it when dropped.
pub(super) fn run(child: Child) -> Run {
    Box::pin(async move {
        match child.wait_with_output().await {
            Ok(Output { status, .. }) if status.success() => Outcome::Finished,
            Ok(Output { status, stderr, .. }) => Outcome::Failed {
                status: status.to_string(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
            },
            Err(err) => Outcome::Unwaited(err),
        }
    })
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
