use nama_plugin_api::PluginId;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt as _;

use super::{
    MAX_PLUGIN_LINE_BYTES, PLUGIN_TARGET, PluginLine, following_current, forget_plugin_lines,
    job_run_span, keep_plugin_lines, layer, plugin_lines, plugin_lines_layer, push_plugin_line,
    service_and_method,
};

#[test]
fn service_and_method_names_the_service_and_method_of_a_connect_path() {
    assert_eq!(
        service_and_method("/nama.v1.SignInService/SignIn"),
        Some(("nama.v1.SignInService", "SignIn"))
    );
}

#[test]
fn service_and_method_is_none_for_a_path_that_is_no_rpc() {
    for path in [
        "/",
        "/index.html",
        "/media/12/master.m3u8",
        "/custom-images/plugin/tmdb",
        "/nama.v1.SignInService/SignIn/more",
    ] {
        assert_eq!(service_and_method(path), None, "{path}");
    }
}

#[test]
fn following_current_joins_the_current_trace_without_holding_its_span_open() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry().with(layer(&provider));
    tracing::subscriber::with_default(subscriber, || {
        let caller = tracing::info_span!("caller");
        let task = caller.in_scope(|| following_current(tracing::info_span!(parent: None, "task")));
        drop(caller);
        let while_running = exporter.get_finished_spans().unwrap();
        assert_eq!(
            while_running.len(),
            1,
            "the caller ended while its task ran on"
        );
        drop(task);
        let ended = exporter.get_finished_spans().unwrap();
        let [caller_data, task_data] = &ended[..] else {
            panic!("expected the caller's and the task's spans, got {ended:#?}");
        };
        assert_eq!(task_data.name, "task");
        assert_eq!(task_data.parent_span_id, caller_data.span_context.span_id());
        assert_eq!(
            task_data.span_context.trace_id(),
            caller_data.span_context.trace_id()
        );
    });
}

fn line(at_ms: i64) -> PluginLine {
    PluginLine {
        at_ms,
        level: Level::INFO,
        text: format!("line {at_ms}"),
        run: None,
    }
}

#[test]
fn an_enabled_plugin_keeps_its_last_100_lines_newest_first_until_disabled() {
    // Its own id: the lines kept are process-wide, and other tests' Plugins log too.
    let plugin = PluginId::from_static("keeps-100");
    keep_plugin_lines(&plugin);
    for at_ms in 0..101 {
        push_plugin_line(&plugin, line(at_ms));
    }

    let kept: Vec<i64> = plugin_lines(&plugin)
        .iter()
        .map(|line| line.at_ms)
        .collect();
    assert_eq!(kept, (1..101).rev().collect::<Vec<_>>());

    forget_plugin_lines(&plugin);
    push_plugin_line(&plugin, line(101));
    assert_eq!(plugin_lines(&plugin), []);
}

#[test]
fn a_plugin_enabled_again_after_a_disable_starts_with_no_lines() {
    let plugin = PluginId::from_static("enabled-again");
    keep_plugin_lines(&plugin);
    push_plugin_line(&plugin, line(0));

    // A racing enabling keeps what the other kept.
    keep_plugin_lines(&plugin);
    assert_eq!(plugin_lines(&plugin), [line(0)]);

    forget_plugin_lines(&plugin);
    keep_plugin_lines(&plugin);
    assert_eq!(plugin_lines(&plugin), []);
}

/// The lines `log` leaves among the Plugin `plugin`'s, newest first, logged through the layer
/// alone.
fn kept_lines(plugin: &PluginId, log: impl FnOnce()) -> Vec<PluginLine> {
    keep_plugin_lines(plugin);
    let subscriber = tracing_subscriber::registry().with(plugin_lines_layer());
    tracing::subscriber::with_default(subscriber, log);
    plugin_lines(plugin)
}

/// The texts of [`kept_lines`].
fn kept_texts(plugin: &PluginId, log: impl FnOnce()) -> Vec<String> {
    let lines = kept_lines(plugin, log).into_iter();
    lines.map(|line| line.text).collect()
}

#[test]
fn a_builtins_span_keeps_its_own_crates_lines_but_not_the_servers_or_a_dependencys() {
    let plugin = PluginId::from_static("span-builtin");
    let kept = kept_texts(&plugin, || {
        let span = tracing::error_span!(target: PLUGIN_TARGET, "plugin", plugin = %plugin);
        span.in_scope(|| {
            tracing::info!(target: "nama_plugin_tmdb", "fetched");
            tracing::error!(plugin = %plugin, "the Store failed");
            tracing::info!(target: "hyper_util::client::legacy", "connecting");
        });
        tracing::info!(target: "nama_plugin_tmdb", "outside its span");
    });

    assert_eq!(kept, ["fetched"]);
}

#[test]
fn a_builtins_settings_check_span_keeps_none_of_its_lines() {
    let plugin = PluginId::from_static("checked-builtin");
    let kept = kept_texts(&plugin, || {
        let span = tracing::error_span!(target: PLUGIN_TARGET, "check", plugin = %plugin);
        span.in_scope(|| tracing::info!(target: "nama_plugin_tmdb", "checking"));
    });

    assert_eq!(kept, Vec::<String>::new());
}

#[test]
fn a_builtins_long_line_is_cut_at_2_kib() {
    let plugin = PluginId::from_static("long-builtin");
    let long = "a".repeat(MAX_PLUGIN_LINE_BYTES + 1);
    let kept = kept_texts(&plugin, || {
        let span = tracing::error_span!(target: PLUGIN_TARGET, "plugin", plugin = %plugin);
        span.in_scope(|| tracing::info!(target: "nama_plugin_tmdb", "{long}"));
    });

    let cut = format!("{} [cut]", "a".repeat(MAX_PLUGIN_LINE_BYTES));
    assert_eq!(kept, [cut]);
}

#[test]
fn an_installed_plugins_line_cut_short_of_2_kib_is_kept_as_it_was_cut() {
    let plugin = PluginId::from_static("cut-installed");
    // `nama.log` cuts a line short of 2 KiB when a secret or a character sits at the cut.
    let cut = format!("{} [cut]", "a".repeat(MAX_PLUGIN_LINE_BYTES - 3));
    let kept = kept_texts(&plugin, || {
        tracing::info!(target: PLUGIN_TARGET, plugin = %plugin, "{cut}");
    });

    assert_eq!(kept, [cut]);
}

#[test]
fn an_installed_plugins_line_named_by_a_str_is_kept() {
    let plugin = PluginId::from_static("named-by-str");
    let kept = kept_texts(&plugin, || {
        tracing::info!(target: PLUGIN_TARGET, plugin = plugin.as_str(), "hello");
    });

    assert_eq!(kept, ["hello"]);
}

#[test]
fn a_line_written_during_a_job_run_carries_its_id_and_one_outside_carries_none() {
    let plugin = PluginId::from_static("runs-jobs");
    let kept = kept_lines(&plugin, || {
        tracing::info!(target: PLUGIN_TARGET, plugin = %plugin, "before");
        job_run_span(7).in_scope(|| {
            tracing::info!(target: PLUGIN_TARGET, plugin = %plugin, "installed");
            let builtin = tracing::error_span!(target: PLUGIN_TARGET, "plugin", plugin = %plugin);
            builtin.in_scope(|| tracing::info!(target: "nama_plugin_upkeep", "built-in"));
        });
    });

    let runs: Vec<(String, Option<i64>)> = kept
        .into_iter()
        .rev()
        .map(|line| (line.text, line.run))
        .collect();
    assert_eq!(
        runs,
        [
            ("before".to_owned(), None),
            ("installed".to_owned(), Some(7)),
            ("built-in".to_owned(), Some(7)),
        ]
    );
}
