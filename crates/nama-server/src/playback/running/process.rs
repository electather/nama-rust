//! ffmpeg's process as a [`Run`].

use tokio::{io::AsyncReadExt, process::Child};
use tokio_util::sync::CancellationToken;

use super::run::{Outcome, Run};

/// `child`, an ffmpeg started with its stderr piped and to be killed on drop, as a run, which
/// kills it and waits for it to exit once `stop` is cancelled, and so stops it when dropped.
// Waited for, as Windows refuses to delete the file it reads, or the folder it writes to, until
// it has exited, and `TerminateProcess` returns before it has.
pub(super) fn run(mut child: Child, stop: CancellationToken) -> Run {
    Box::pin(async move {
        let pipe = child.stderr.take();
        let mut stderr = vec![];
        let read_stderr = async {
            if let Some(mut pipe) = pipe {
                // What it wrote before a read failed is all there is to report.
                let _: std::io::Result<usize> = pipe.read_to_end(&mut stderr).await;
            }
        };
        let waited = tokio::select! {
            (waited, ()) = async { tokio::join!(child.wait(), read_stderr) } => waited,
            () = stop.cancelled() => {
                return child.kill().await.map_or_else(Outcome::Unwaited, |()| Outcome::Stopped);
            }
        };
        match waited {
            Ok(status) if status.success() => Outcome::Finished,
            Ok(status) => Outcome::Failed {
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
