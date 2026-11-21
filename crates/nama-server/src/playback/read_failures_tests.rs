use std::io;

use super::{Read, ReadFailure};
use crate::ffmpeg::{Program, RunError};

fn failed(stderr: &str) -> RunError {
    RunError::Failed(Program::Ffmpeg, stderr.to_owned())
}

#[tokio::test]
async fn a_failed_read_means_what_its_error_kind_of_read_and_file_say() {
    let folder = tempfile::tempdir().unwrap();
    let there = folder.path().join("there.mkv");
    std::fs::write(&there, "a byte").unwrap();
    let gone = folder.path().join("gone.mkv");
    let refused = [
        "av_interleaved_write_frame(): No space left on device",
        "[mp4] 0.m4s: Disk quota exceeded",
        "Error writing trailer: Read-only file system",
    ];
    let mut rows: Vec<(RunError, Read, &_, ReadFailure)> = vec![
        (
            RunError::Unavailable(Program::Ffmpeg),
            Read::WholeFile,
            &there,
            ReadFailure::NoProgram,
        ),
        (
            RunError::Run(Program::Ffprobe, io::Error::other("exec format error")),
            Read::Probe,
            &there,
            ReadFailure::WontRun,
        ),
        (
            failed("Invalid data found when processing input"),
            Read::WholeFile,
            &there,
            ReadFailure::Lasting,
        ),
        (
            failed("Invalid data found when processing input"),
            Read::Probe,
            &gone,
            ReadFailure::NotTheFile,
        ),
        (
            RunError::TimedOut(Program::Ffprobe),
            Read::Probe,
            &there,
            ReadFailure::Passing,
        ),
        (
            RunError::TimedOut(Program::Ffmpeg),
            Read::WholeFile,
            &there,
            ReadFailure::Lasting,
        ),
    ];
    for stderr in refused {
        for file in [&there, &gone] {
            rows.push((
                failed(stderr),
                Read::WholeFile,
                file,
                ReadFailure::NotTheFile,
            ));
        }
    }

    for (error, read, file, meaning) in rows {
        let row = format!("{error} on a {read:?} read of {}", file.display());
        assert_eq!(ReadFailure::of(&error, read, file).await, meaning, "{row}");
    }
}
