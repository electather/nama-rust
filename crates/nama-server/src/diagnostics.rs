//! Diagnostics: the spans the Server records about itself, and the web app's that it forwards,
//! exported only to an endpoint its admin sets (ADR 0054), and each Plugin's recent Plugin Log
//! lines, kept for its Plugins page (ADR 0061). What a span may carry is a rule in
//! `crates/nama-server/AGENTS.md`.

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    env,
    fmt::{self, Write as _},
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::{Duration, Instant, SystemTime},
};

use axum::{
    body::Bytes,
    extract::Request,
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    middleware::Next,
    response::Response,
};
use connectrpc::{
    ConnectError, Interceptor, NextStream, PayloadStream,
    interceptor::{Next as NextUnary, StreamRequest, StreamResponse, UnaryRequest, UnaryResponse},
};
use nama_plugin_api::PluginId;
use opentelemetry::{
    KeyValue,
    propagation::TextMapPropagator as _,
    trace::{Span as _, SpanKind, Status, Tracer as _, TracerProvider as _},
};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{SdkTracer, SdkTracerProvider},
};
use tracing::{
    Dispatch, Event, Instrument as _, Level, Span, Subscriber,
    dispatcher::WeakDispatch,
    field::{Field, Visit},
    span::{Attributes, Id},
};
use tracing_opentelemetry::{OpenTelemetrySpanExt as _, SetParentError, get_otel_context};
use tracing_subscriber::{
    EnvFilter, Layer,
    filter::{FilterExt as _, filter_fn},
    layer::Context,
    registry::LookupSpan,
};

use crate::db;

/// The target sqlx logs each query it ran under, once it has run.
const SQL_TARGET: &str = "sqlx::query";

/// The target of a Plugin's lines and of the span naming a Built-in on stdout.
pub(crate) const PLUGIN_TARGET: &str = "nama::plugin";

/// The name of the span under [`PLUGIN_TARGET`] that a call to an enabled Built-in runs in. A
/// settings check's span is named apart: its lines come from no enablement, so no Plugin Log keeps
/// them (ADR 0061).
pub(crate) const PLUGIN_SPAN: &str = "plugin";

/// The name of the span under [`PLUGIN_TARGET`] a Job Run runs in, which [`job_run_span`] makes.
const JOB_RUN_SPAN: &str = "job run";

/// How a Built-in's own target starts, since its package is `nama-plugin-<folder>`
/// (`plugins/AGENTS.md`).
const BUILTIN_TARGET_PREFIX: &str = "nama_plugin_";

/// The most of a Plugin Log line kept, in bytes, so a Plugin can't fill the log with one.
pub(crate) const MAX_PLUGIN_LINE_BYTES: usize = 2048;

/// How many of each Plugin's lines its Plugins page shows.
const KEPT_PLUGIN_LINES: usize = 100;

/// Each enabled Plugin's last [`KEPT_PLUGIN_LINES`], oldest first. Process-wide, as the subscriber
/// filling it is, and in memory only (ADR 0061).
static PLUGIN_LINES: Mutex<BTreeMap<PluginId, VecDeque<PluginLine>>> = Mutex::new(BTreeMap::new());

/// A line of a Plugin's Plugin Log, as its Plugins page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginLine {
    pub(crate) at_ms: i64,
    pub(crate) level: Level,
    /// Cut at [`MAX_PLUGIN_LINE_BYTES`]. An Installed Plugin's arrives with its secrets blanked.
    pub(crate) text: String,
    /// The id of the Job Run it was written during, if any.
    pub(crate) run: Option<i64>,
}

/// The layer that keeps each enabled Plugin's last Plugin Log lines for its Plugins page. Filter
/// it as stdout is, so it keeps the lines stdout shows.
#[must_use]
pub fn plugin_lines_layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    KeepPluginLines
}

/// The Plugin `plugin`'s last lines, newest first.
pub(crate) fn plugin_lines(plugin: &PluginId) -> Vec<PluginLine> {
    let kept = PLUGIN_LINES.lock().unwrap_or_else(PoisonError::into_inner);
    kept.get(plugin)
        .map_or_default(|lines| lines.iter().rev().cloned().collect())
}

/// Starts keeping the Plugin `plugin`'s lines, as it is being enabled. Lines it already has are
/// kept: they are another enabling's, racing this one, since disabling it forgets them.
pub(crate) fn keep_plugin_lines(plugin: &PluginId) {
    let mut kept = PLUGIN_LINES.lock().unwrap_or_else(PoisonError::into_inner);
    kept.entry(plugin.clone()).or_default();
}

/// Forgets the Plugin `plugin`'s lines and keeps no more, once it is disabled or failed to enable.
pub(crate) fn forget_plugin_lines(plugin: &PluginId) {
    let mut kept = PLUGIN_LINES.lock().unwrap_or_else(PoisonError::into_inner);
    kept.remove(plugin);
}

/// The span the Job Run `run` calls its Plugin in, so each line the Plugin writes in it carries
/// `run` on stdout and among its kept lines. Under [`PLUGIN_TARGET`], so it isn't exported.
pub(crate) fn job_run_span(run: i64) -> Span {
    // `error`, as `plugin` spans are, so a level filter that keeps any line keeps the span.
    tracing::error_span!(target: PLUGIN_TARGET, JOB_RUN_SPAN, job_run = run)
}

/// Adds `line` to `plugin`'s last lines, dropping its oldest past [`KEPT_PLUGIN_LINES`], if its
/// lines are kept: a line from a Plugin that isn't enabled, such as one checking its settings, is
/// dropped. A line carries no enablement, so one from a call still running after a disable lands
/// in the next enabling's lines.
fn push_plugin_line(plugin: &PluginId, line: PluginLine) {
    let mut kept = PLUGIN_LINES.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(lines) = kept.get_mut(plugin) else {
        return;
    };
    if lines.len() == KEPT_PLUGIN_LINES {
        lines.pop_front();
    }
    lines.push_back(line);
}

/// Cuts `line` at [`MAX_PLUGIN_LINE_BYTES`] and marks it as cut.
pub(crate) fn cut_plugin_line(line: &mut String) {
    line.truncate(line.floor_char_boundary(MAX_PLUGIN_LINE_BYTES));
    line.push_str(" [cut]");
}

/// The layer that exports the Server's spans through `provider`, each SQL query among them.
///
/// Spans pass [`exported_filter`]. Logged events never reach it, so a span carries none.
#[must_use]
pub fn layer<S>(provider: &SdkTracerProvider) -> impl Layer<S> + use<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let tracer = provider.tracer(env!("CARGO_PKG_NAME"));
    let queries = SqlSpans {
        tracer: tracer.clone(),
        dispatch: OnceLock::new(),
    }
    .with_filter(filter_fn(|metadata| metadata.target() == SQL_TARGET));
    // Lines logged for stdout quote file paths and names, which a span mustn't carry (ADR 0054).
    // The span naming a Built-in is for stdout: in a trace, the `plugin call` span inside it names
    // the Plugin already.
    let spans_only = exported_filter().and(filter_fn(|metadata| {
        metadata.is_span() && metadata.target() != PLUGIN_TARGET
    }));
    tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(spans_only)
        .and_then(queries)
}

/// What is exported: `info`, or whatever `RUST_LOG` asks for, never Extism's own Plugin logs.
#[must_use]
pub fn exported_filter() -> EnvFilter {
    let asked = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());
    // Extism logs an Installed Plugin's error, and the lines it writes through Extism's own log
    // functions, as they are, which may quote a secret setting; the Server logs both itself with
    // secrets blanked (ADR 0061). Forced here, unlike stdout, since this leaves the machine.
    EnvFilter::builder().parse_lossy(format!("{asked},extism::plugin=off,extism::pdk=off"))
}

/// Runs a ConnectRPC call in a span named after its service and method, continuing the trace a
/// W3C `traceparent` header names. Outermost, so the SQL that identifies the caller is in it too.
pub(crate) async fn trace_call(mut request: Request, next: Next) -> Response {
    let Some((service, method)) = service_and_method(request.uri().path()) else {
        return next.run(request).await;
    };
    let span = tracing::info_span!(
        "rpc",
        otel.name = format!("{service}/{method}"),
        otel.kind = "server",
        rpc.system = "connect_rpc",
        rpc.service = service,
        rpc.method = method,
    );
    let parent = TraceContextPropagator::new().extract(&HeaderExtractor(request.headers()));
    // Fails only when nothing exports spans, so there's no trace to continue.
    let _: Result<(), SetParentError> = span.set_parent(parent);
    // For `RecordFailures` and the access layer, which mark it failed.
    request.extensions_mut().insert(span.clone());
    let response = next.run(request).instrument(span.clone()).await;
    let status = response.status();
    span.set_attribute("http.response.status_code", i64::from(status.as_u16()));
    // Some refusals, such as a session that can't be read, come before any layer names a kind.
    if !status.is_success() {
        span.set_status(Status::error(""));
    }
    response
}

/// The service and method of an RPC's `path`, such as `/nama.v1.SignInService/SignIn`, or `None`
/// for a path that isn't one, such as media's.
fn service_and_method(path: &str) -> Option<(&str, &str)> {
    let (service, method) = path.strip_prefix('/')?.split_once('/')?;
    (service.starts_with("nama.") && !method.contains('/')).then_some((service, method))
}

/// `span`, made with `parent: None`, put in the current span's trace, for a task spawned from the
/// current span that may outlive it: as a child it would hold the current span open, stretching
/// that span's duration to the task's.
pub(crate) fn following_current(span: Span) -> Span {
    // Fails only when nothing exports spans, so there's no trace to join.
    let _: Result<(), SetParentError> = span.set_parent(Span::current().context());
    span
}

/// Marks the RPC span [`trace_call`] left in a request's `extensions` failed with `err`'s code.
pub(crate) fn rpc_failed(extensions: &axum::http::Extensions, err: &ConnectError) {
    mark_if_failed(extensions.get::<Span>(), Some(err));
}

/// Marks `span`, an RPC's, failed with `err`'s Connect code, if there is an error. A stream's
/// error once it has started isn't seen here.
fn mark_if_failed(span: Option<&Span>, err: Option<&ConnectError>) {
    if let (Some(span), Some(err)) = (span, err) {
        failed(span, err.code.as_str());
    }
}

/// Marks `span` failed, with `kind` saying how, such as a Connect code.
pub(crate) fn failed(span: &Span, kind: &'static str) {
    // Described by `error.type`, not here: SDKs disagree on which of two descriptions is kept.
    span.set_status(Status::error(""));
    span.set_attribute("error.type", kind);
}

/// Marks a ConnectRPC call's span failed when its handler fails, with the error's code.
pub(crate) struct RecordFailures;

#[connectrpc::async_trait]
impl Interceptor for RecordFailures {
    async fn intercept_unary(
        &self,
        req: UnaryRequest,
        next: NextUnary<'_>,
    ) -> Result<UnaryResponse, ConnectError> {
        let span = req.ctx.extensions().get::<Span>().cloned();
        let called = next.run(req).await;
        mark_if_failed(span.as_ref(), called.as_ref().err());
        called
    }

    async fn intercept_streaming(
        &self,
        req: StreamRequest,
        inbound: PayloadStream,
        next: NextStream<'_>,
    ) -> Result<StreamResponse, ConnectError> {
        let span = req.ctx.extensions().get::<Span>().cloned();
        let called = next.run(req, inbound).await;
        mark_if_failed(span.as_ref(), called.as_ref().err());
        called
    }
}

/// Turns each query sqlx logs into a span, a child of the span that ran it: sqlx records none of
/// its own, only an event once the query has run, saying how long it took.
struct SqlSpans {
    tracer: SdkTracer,
    /// The subscriber this is part of, to find the exported span a query ran in.
    dispatch: OnceLock<WeakDispatch>,
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
        self.tracer
            .span_builder(query.summary)
            .with_kind(SpanKind::Client)
            .with_start_time(ended.checked_sub(query.elapsed).unwrap_or(ended))
            .with_attributes([
                KeyValue::new("db.system.name", "sqlite"),
                KeyValue::new("db.response.returned_rows", query.rows_returned),
                KeyValue::new("db.response.affected_rows", query.rows_affected),
            ])
            .start_with_context(&self.tracer, &parent)
            .end_with_timestamp(ended);
    }
}

/// What sqlx logs of a query that has run. Its summary is the statement's first words, which hold
/// no values: every query binds them.
#[derive(Default)]
struct Query {
    summary: String,
    rows_returned: i64,
    rows_affected: i64,
    elapsed: Duration,
}

impl Visit for Query {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "summary" {
            value.clone_into(&mut self.summary);
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

/// See [`plugin_lines_layer`].
struct KeepPluginLines;

/// The Built-in a [`PLUGIN_SPAN`] names, kept in its extensions.
struct SpanPlugin(PluginId);

/// The Job Run a [`JOB_RUN_SPAN`] names, kept in its extensions.
struct SpanRun(i64);

impl<S> Layer<S> for KeepPluginLines
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let metadata = attrs.metadata();
        if metadata.target() != PLUGIN_TARGET {
            return;
        }
        let Some(span) = ctx.span(id) else {
            return;
        };
        match metadata.name() {
            PLUGIN_SPAN => {
                let mut fields = LineFields::default();
                attrs.record(&mut fields);
                if let Some(plugin) = fields.plugin {
                    span.extensions_mut().insert(SpanPlugin(plugin));
                }
            }
            JOB_RUN_SPAN => {
                let mut run = RunField::default();
                attrs.record(&mut run);
                if let RunField(Some(run)) = run {
                    span.extensions_mut().insert(SpanRun(run));
                }
            }
            _ => {}
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut fields = LineFields::default();
        let (plugin, text) = if metadata.target() == PLUGIN_TARGET {
            // An Installed Plugin's line, named in its own field and cut already.
            event.record(&mut fields);
            let Some(plugin) = fields.plugin.take() else {
                return;
            };
            (plugin, fields.text())
        } else {
            // Only the Built-in's own: the Server's, such as its Kernel's failures, and those of a
            // crate it calls, such as its HTTP client's, aren't the Plugin's to show.
            if !metadata.target().starts_with(BUILTIN_TARGET_PREFIX) {
                return;
            }
            let Some(plugin) = innermost(event, &ctx, |SpanPlugin(plugin)| plugin.clone()) else {
                return;
            };
            event.record(&mut fields);
            let mut text = fields.text();
            if text.len() > MAX_PLUGIN_LINE_BYTES {
                cut_plugin_line(&mut text);
            }
            (plugin, text)
        };
        let line = PluginLine {
            at_ms: db::now_ms(),
            level: *metadata.level(),
            text,
            run: innermost(event, &ctx, |&SpanRun(run)| run),
        };
        push_plugin_line(&plugin, line);
    }
}

/// What `read` reads of the innermost span around `event` that keeps a `T` in its extensions.
fn innermost<S, T, R>(event: &Event<'_>, ctx: &Context<'_, S>, read: impl Fn(&T) -> R) -> Option<R>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    T: Send + Sync + 'static,
{
    ctx.event_scope(event)?
        .find_map(|span| span.extensions().get::<T>().map(&read))
}

/// The `job_run` field of a [`JOB_RUN_SPAN`].
#[derive(Default)]
struct RunField(Option<i64>);

impl Visit for RunField {
    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "job_run" {
            self.0 = Some(value);
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn fmt::Debug) {}
}

/// A line's fields as stdout shows them: its message, then each other field but `plugin`, which
/// names its Plugin.
#[derive(Default)]
struct LineFields {
    plugin: Option<PluginId>,
    message: String,
    /// ` name=value` for each other field.
    rest: String,
}

impl LineFields {
    fn text(self) -> String {
        self.message + &self.rest
    }
}

impl Visit for LineFields {
    // `plugin = %id` reaches `record_debug`, but `plugin = id.as_str()` here, whose `Debug` would
    // quote it.
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "plugin" {
            self.plugin = Some(PluginId::from(value.to_owned()));
        } else {
            self.record_debug(field, &value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // Writing to a `String` can't fail.
        let _: fmt::Result = match field.name() {
            "plugin" => {
                self.plugin = Some(PluginId::from(format!("{value:?}")));
                Ok(())
            }
            "message" => write!(self.message, "{value:?}"),
            name => write!(self.rest, " {name}={value:?}"),
        };
    }
}

/// How many times one Device may post its spans in [`POSTS_WINDOW`]: its exporter sends a batch
/// every 5 seconds, or sooner once 512 spans wait, so this leaves room for a busy page's bursts.
const POSTS_PER_WINDOW: u32 = 60;

const POSTS_WINDOW: Duration = Duration::from_mins(1);

/// How long the collector has to take a post before it fails, so a stuck one holds no request.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(10);

/// Forwards the web app's spans, as OTLP/HTTP, to where the Server exports its own, storing
/// nothing (ADR 0054), and limits how often each Device may post them.
#[derive(Clone)]
pub(crate) struct BrowserSpans {
    traces_url: String,
    client: reqwest::Client,
    /// When each Device's current window started, and how many posts it has made in it.
    posts: Arc<Mutex<HashMap<i64, (Instant, u32)>>>,
}

impl BrowserSpans {
    /// Forwards to the OTLP `endpoint`'s traces path.
    pub(crate) fn new(endpoint: &str) -> Self {
        // ponytail: ignores `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` and `OTEL_EXPORTER_OTLP_HEADERS`,
        // which the Server's own exporter reads; read them once a collector needs a key.
        let traces_url = format!("{}/v1/traces", endpoint.trim_end_matches('/'));
        Self {
            traces_url,
            client: reqwest::Client::new(),
            posts: Arc::default(),
        }
    }

    /// Forwards `body`, of `content_type`, as it is, for the Device `device`, and answers with the
    /// collector's status: 429 instead if that Device posted too often lately, as of `now`, or 502
    /// if the collector can't be reached.
    pub(crate) async fn forward(
        &self,
        device: i64,
        content_type: Option<HeaderValue>,
        body: Bytes,
        now: Instant,
    ) -> StatusCode {
        if !self.admit(device, now) {
            return StatusCode::TOO_MANY_REQUESTS;
        }
        let mut request = self
            .client
            .post(&self.traces_url)
            .timeout(FORWARD_TIMEOUT)
            .body(body);
        if let Some(content_type) = content_type {
            request = request.header(CONTENT_TYPE, content_type);
        }
        match request.send().await {
            Ok(response) => response.status(),
            Err(err) => {
                tracing::warn!("could not forward the web app's Diagnostics: {err}");
                StatusCode::BAD_GATEWAY
            }
        }
    }

    /// Counts a post by `device` at `now`, and whether it is within the limit.
    fn admit(&self, device: i64, now: Instant) -> bool {
        // Every change leaves the counts consistent, so a panic elsewhere doesn't spoil them.
        let mut posts = self.posts.lock().unwrap_or_else(PoisonError::into_inner);
        posts.retain(|_, (started, _)| now.saturating_duration_since(*started) < POSTS_WINDOW);
        let (_, count) = posts.entry(device).or_insert((now, 0));
        *count = count.saturating_add(1);
        *count <= POSTS_PER_WINDOW
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
