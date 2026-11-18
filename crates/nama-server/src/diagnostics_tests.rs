use nama_plugin_api::PluginId;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt as _;

use super::{
    PluginLine, following_current, forget_plugin_lines, keep_plugin_line, keep_plugin_lines, layer,
    plugin_lines, service_and_method,
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
    }
}

#[test]
fn an_enabled_plugin_keeps_its_last_100_lines_newest_first_until_disabled() {
    // Its own id: the lines kept are process-wide, and other tests' Plugins log too.
    let plugin = PluginId::from_static("keeps-100");
    keep_plugin_lines(&plugin);
    for at_ms in 0..101 {
        keep_plugin_line(&plugin, line(at_ms));
    }

    let kept: Vec<i64> = plugin_lines(&plugin)
        .iter()
        .map(|line| line.at_ms)
        .collect();
    assert_eq!(kept, (1..101).rev().collect::<Vec<_>>());

    forget_plugin_lines(&plugin);
    keep_plugin_line(&plugin, line(101));
    assert_eq!(plugin_lines(&plugin), []);
}

#[test]
fn a_plugin_enabled_again_starts_with_no_lines() {
    let plugin = PluginId::from_static("enabled-again");
    keep_plugin_lines(&plugin);
    keep_plugin_line(&plugin, line(0));

    keep_plugin_lines(&plugin);

    assert_eq!(plugin_lines(&plugin), []);
}
