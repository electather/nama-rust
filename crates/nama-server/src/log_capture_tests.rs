use std::thread;

use super::CapturedLog;

/// The callsite only the test below reaches, from two threads.
fn warn_of_the_race() {
    tracing::warn!("the race's warning");
}

#[test]
fn a_line_another_thread_without_a_subscriber_reached_first_is_still_captured() {
    let log = CapturedLog::start();

    thread::spawn(warn_of_the_race).join().unwrap();
    warn_of_the_race();

    let log = log.finish();
    let warned = log.lines().filter(|line| line.contains("WARN")).count();
    assert_eq!(warned, 1, "{log}");
}
