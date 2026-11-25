//! The spans of the queries sqlx logs.

use std::{
    fmt,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use opentelemetry::{
    KeyValue,
    trace::{Span as _, SpanKind, Tracer as _},
};
use opentelemetry_sdk::trace::SdkTracer;
use tracing::{
    Dispatch, Event, Subscriber,
    dispatcher::WeakDispatch,
    field::{Field, Visit},
};
use tracing_opentelemetry::get_otel_context;
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

/// Turns each query sqlx logs into a span, a child of the span that ran it: sqlx records none of
/// its own, only an event once the query has run, saying how long it took.
pub(super) struct SqlSpans {
    pub(super) tracer: SdkTracer,
    /// The subscriber this is part of, to find the exported span a query ran in.
    pub(super) dispatch: OnceLock<WeakDispatch>,
}

impl<S> Layer<S> for SqlSpans
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_register_dispatch(&self, subscriber: &Dispatch) {
        // A subscriber is registered once; a second registration would be the same one.
        let _: Result<(), WeakDispatch> = self.dispatch.set(subscriber.downgrade());
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut query = Query::default();
        event.record(&mut query);
        let dispatch = self.dispatch.get().and_then(WeakDispatch::upgrade);
        let parent = ctx
            .current_span()
            .id()
            .zip(dispatch)
            .and_then(|(id, dispatch)| get_otel_context(id, &dispatch))
            .unwrap_or_default();
        // Real time, not `db::now`: the exception to "One clock" in `crates/nama-server/AGENTS.md`.
        let ended = SystemTime::now();
        let text = query.text();
        self.tracer
            .span_builder(query.summary)
            .with_kind(SpanKind::Client)
            .with_start_time(ended.checked_sub(query.elapsed).unwrap_or(ended))
            .with_attributes([
                KeyValue::new("db.system.name", "sqlite"),
                KeyValue::new("db.query.text", text),
                KeyValue::new("db.response.returned_rows", query.rows_returned),
                KeyValue::new("db.response.affected_rows", query.rows_affected),
            ])
            .start_with_context(&self.tracer, &parent)
            .end_with_timestamp(ended);
    }
}

/// What sqlx logs of a query that has run. Its summary is the statement's first words, and its
/// statement the whole of it, neither holding values: every query binds them.
#[derive(Default)]
struct Query {
    summary: String,
    /// The statement, empty when the summary is all of it.
    statement: String,
    rows_returned: i64,
    rows_affected: i64,
    elapsed: Duration,
}

impl Query {
    /// The statement on one line, so the backend groups each query's runs however it was indented.
    fn text(&self) -> String {
        let statement = if self.statement.is_empty() {
            &self.summary
        } else {
            &self.statement
        };
        statement.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

impl Visit for Query {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "summary" => value.clone_into(&mut self.summary),
            "db.statement" => value.clone_into(&mut self.statement),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let value = i64::try_from(value).unwrap_or(i64::MAX);
        match field.name() {
            "rows_returned" => self.rows_returned = value,
            "rows_affected" => self.rows_affected = value,
            _ => {}
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "elapsed_secs" {
            self.elapsed = Duration::try_from_secs_f64(value).unwrap_or_default();
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn fmt::Debug) {}
}
