// The fakes are shell scripts.
#![cfg(unix)]

use std::{path::Path, process::Stdio, time::Duration};

use tokio_util::sync::CancellationToken;

use super::{KEPT_STDERR, Outcome, run};
use crate::ffmpeg::fake;

/// The fake ffmpeg `body` in `bin`, started as `HlsRun` starts ffmpeg, with `arg`.
fn spawn(bin: &Path, body: &str, arg: &Path) -> tokio::process::Child {
    let program = fake::script(bin, "ffmpeg", body);
    tokio::process::Command::new(program.path)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

/// An ffmpeg that writes the file it's given after 0.3 seconds.
const WRITES_LATE: &str = "#!/bin/sh\nsleep 0.3\ntouch \"$1\"\n";

#[tokio::test]
async fn a_run_dropped_kills_its_ffmpeg() {
    let bin = tempfile::tempdir().unwrap();
    let (late, waited) = (bin.path().join("late"), bin.path().join("waited"));
    let ffmpeg = spawn(bin.path(), WRITES_LATE, &late);

    drop(run(ffmpeg, CancellationToken::new()));

    // One run to its end alongside, so the dropped one had as long to write its file.
    run(
        spawn(bin.path(), WRITES_LATE, &waited),
        CancellationToken::new(),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(waited.exists());
    assert!(!late.exists());
}

#[tokio::test]
async fn a_stopped_run_ends_once_its_ffmpeg_has_exited() {
    let bin = tempfile::tempdir().unwrap();
    let started = bin.path().join("started");
    let ffmpeg = spawn(bin.path(), "#!/bin/sh\ntouch \"$1\"\nsleep 30\n", &started);
    let pid = ffmpeg.id().unwrap();
    let stop = CancellationToken::new();
    let running = tokio::spawn(run(ffmpeg, stop.clone()));
    // Stopped only once it runs: one killed before it starts may be reaped before `ps` looks,
    // even by a run that doesn't wait for it.
    while !started.exists() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    stop.cancel();
    let outcome = running.await.unwrap();

    assert!(matches!(outcome, Outcome::Stopped), "{outcome:?}");
    // `ps` lists a zombie, which only a wait reaps, so it lists nothing only once that's done.
    let ps = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    assert!(
        ps.stdout.trim_ascii().is_empty(),
        "{}",
        String::from_utf8_lossy(&ps.stdout)
    );
}

#[tokio::test]
async fn a_failed_run_carries_its_status_and_stderr() {
    let bin = tempfile::tempdir().unwrap();
    let body = "#!/bin/sh\necho '[mp4] 0.m4s: No space left on device' >&2\nexit 3\n";
    let ffmpeg = spawn(bin.path(), body, bin.path());

    let outcome = run(ffmpeg, CancellationToken::new()).await;

    let Outcome::Failed { status, stderr } = outcome else {
        panic!("{outcome:?}");
    };
    assert!(status.contains('3'), "{status}");
    assert_eq!(stderr.trim(), "[mp4] 0.m4s: No space left on device");
}

#[tokio::test]
async fn a_run_that_exits_cleanly_finishes() {
    let bin = tempfile::tempdir().unwrap();
    let ffmpeg = spawn(bin.path(), "#!/bin/sh\nexit 0\n", bin.path());

    assert!(matches!(
        run(ffmpeg, CancellationToken::new()).await,
        Outcome::Finished
    ));
}

#[tokio::test]
async fn a_failed_run_keeps_only_the_tail_of_a_long_stderr() {
    let bin = tempfile::tempdir().unwrap();
    // 100,000 `a`s, then a last line, past what's kept.
    let body = "#!/bin/sh\nhead -c 100000 /dev/zero | tr '\\0' a >&2\necho ' last' >&2\nexit 1\n";
    let ffmpeg = spawn(bin.path(), body, bin.path());

    let outcome = run(ffmpeg, CancellationToken::new()).await;

    let Outcome::Failed { stderr, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(stderr.len(), KEPT_STDERR);
    assert!(stderr.starts_with('a') && stderr.ends_with("aaa last\n"));
}
