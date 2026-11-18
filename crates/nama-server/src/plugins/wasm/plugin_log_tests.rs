use nama_plugin_api::PluginId;
use tracing::Level;

use super::PluginLog;
use crate::log_capture::CapturedLog;

fn plugin_log(secrets: &[&str]) -> PluginLog {
    let secrets = secrets.iter().map(|&secret| secret.to_owned()).collect();
    PluginLog::new(PluginId::from("radarr".to_owned()), secrets)
}

/// What `write` logs when called once for each of `lines`, all at `now_ms`.
fn logged(log: &PluginLog, lines: &[(Level, &str)], now_ms: i64) -> String {
    let captured = CapturedLog::start();
    for &(level, text) in lines {
        log.write(level, text.as_bytes(), now_ms);
    }
    captured.finish()
}

#[test]
fn a_line_is_logged_at_its_level_under_nama_plugin_naming_its_plugin() {
    let log = plugin_log(&[]);

    let logged = logged(&log, &[(Level::WARN, "Radarr answered 503")], 0);

    assert!(
        logged.contains(" WARN nama::plugin: Radarr answered 503 plugin=radarr"),
        "{logged}"
    );
}

#[test]
fn a_newline_in_a_line_is_escaped_so_it_cannot_forge_a_server_line() {
    let log = plugin_log(&[]);
    let forged = "ok\n2026-11-17T10:00:00Z ERROR nama_server::users: forged";

    let logged = logged(&log, &[(Level::WARN, forged)], 0);

    let escaped = r"ok\n2026-11-17T10:00:00Z ERROR nama_server::users: forged plugin=radarr";
    assert!(logged.contains(escaped), "{logged}");
    assert!(
        !logged
            .lines()
            .any(|line| line.starts_with("2026-11-17T10:00:00Z")),
        "{logged}"
    );
}

#[test]
fn a_secret_settings_value_is_blanked_in_a_line() {
    let log = plugin_log(&["s3cr3t-key"]);

    let logged = logged(&log, &[(Level::INFO, "GET /api?apikey=s3cr3t-key")], 0);

    assert!(
        logged.contains("GET /api?apikey=… plugin=radarr"),
        "{logged}"
    );
    assert!(!logged.contains("s3cr3t-key"), "{logged}");
}

#[test]
fn a_line_over_2_kib_is_cut_there_and_marked_as_cut() {
    let log = plugin_log(&[]);
    // A two-byte character straddles the 2 KiB mark, so the cut falls before it.
    let too_long = format!("{}é{}", "a".repeat(2047), "b".repeat(100));

    let logged = logged(&log, &[(Level::INFO, &too_long)], 0);

    let cut = format!(": {} [cut] plugin=radarr", "a".repeat(2047));
    assert!(logged.contains(&cut), "{logged}");
}

#[test]
fn a_plugin_logs_a_burst_of_50_lines_then_10_a_second() {
    let log = plugin_log(&[]);
    let burst = [(Level::INFO, "burst line"); 60];
    let later = [(Level::INFO, "later line"); 11];

    let logged_at_once = logged(&log, &burst, 0);
    let logged_a_second_later = logged(&log, &later, 1000);

    assert_eq!(logged_at_once.matches("burst line").count(), 50);
    assert_eq!(logged_a_second_later.matches("later line").count(), 10);
}

#[test]
fn a_line_the_level_filter_drops_spends_nothing_from_the_bucket() {
    let log = plugin_log(&[]);
    // `CapturedLog` logs at `info` and above.
    let filtered = [(Level::DEBUG, "filtered line"); 60];
    let burst = [(Level::INFO, "burst line"); 50];

    let logged = logged(&log, &[&filtered[..], &burst[..]].concat(), 0);

    assert_eq!(logged.matches("filtered line").count(), 0);
    assert_eq!(logged.matches("burst line").count(), 50);
}

#[test]
fn dropped_lines_are_counted_in_one_warn_a_minute_naming_the_plugin() {
    let log = plugin_log(&[]);
    let burst = [(Level::INFO, "burst line"); 60];
    let line = [(Level::INFO, "line")];

    let logged_within_the_minute = [logged(&log, &burst, 0), logged(&log, &line, 59_999)].concat();
    let logged_a_minute_on = logged(&log, &[line, line].concat(), 60_000);

    assert!(
        !logged_within_the_minute.contains("dropped"),
        "{logged_within_the_minute}"
    );
    let warning = "dropped 10 Plugin Log lines over its limit of 10 a second plugin=radarr";
    assert!(logged_a_minute_on.contains(warning), "{logged_a_minute_on}");
    let warnings = logged_a_minute_on
        .lines()
        .filter(|logged| logged.contains("dropped"));
    assert_eq!(warnings.count(), 1, "{logged_a_minute_on}");
}

#[test]
fn a_secret_a_long_lines_cut_runs_through_shows_none_of_it() {
    let log = plugin_log(&["s3cr3t-key"]);
    // Blanking shrinks each secret, so the line's tail comes within 2 KiB once blanked.
    let secrets = "s3cr3t-key".repeat(1000);

    let logged = logged(&log, &[(Level::INFO, &secrets)], 0);

    assert!(logged.contains("… [cut] plugin=radarr"), "{logged}");
    assert!(!logged.contains("s3cr"), "{logged}");
}

#[test]
fn lines_dropped_since_the_last_warn_are_counted_when_the_plugin_log_ends() {
    let log = plugin_log(&[]);
    let burst = [(Level::INFO, "burst line"); 60];
    let logged_burst = logged(&log, &burst, 0);

    let captured = CapturedLog::start();
    drop(log);
    let logged_at_end = captured.finish();

    assert!(!logged_burst.contains("dropped"), "{logged_burst}");
    let warning = "dropped 10 Plugin Log lines over its limit of 10 a second plugin=radarr";
    assert!(logged_at_end.contains(warning), "{logged_at_end}");
}
