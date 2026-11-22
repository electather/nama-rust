//! The `nama-hub` binary.

use std::{
    env,
    future::Future,
    io::{self, IsTerminal},
    process::ExitCode,
};

use clap::Parser;
use nama_hub::{config::Config, db};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{ExporterBuildError, LogExporter, SpanExporter};
use opentelemetry_sdk::{logs::SdkLoggerProvider, trace::SdkTracerProvider};
use tokio::net::TcpListener;
use tracing::{Subscriber, error, info, warn};
use tracing_subscriber::{
    EnvFilter, Layer,
    filter::{FilterExt as _, filter_fn},
    layer::SubscriberExt as _,
    registry::LookupSpan,
    util::SubscriberInitExt as _,
};

/// The standard variable naming where Diagnostics are exported; unset, nothing is (ADR 0054).
const OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    after_help = "Configured by environment variables only: NAMA_HUB_DATABASE_URL, the \
                  PostgreSQL database as a postgres:// URL, and NAMA_HUB_LISTEN, the address to \
                  listen on (0.0.0.0:7981 by default)."
)]
struct Args;

#[tokio::main]
async fn main() -> ExitCode {
    Args::parse();
    let exporting = init_tracing();
    let exit = run().await;
    if let Some(exporting) = exporting {
        exporting.shutdown();
    }
    exit
}

/// Runs the Hub until it is told to stop.
async fn run() -> ExitCode {
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            error!("could not load config: {err}");
            return ExitCode::FAILURE;
        }
    };
    // Listen before announcing startup, so a Ctrl-C right after the log line isn't missed.
    let shutdown = match shutdown_signal() {
        Ok(shutdown) => shutdown,
        Err(err) => {
            error!("could not listen for shutdown signals: {err}");
            return ExitCode::FAILURE;
        }
    };
    info!(listen = %config.listen, "loaded config");

    let db = match db::connect(&config.database_url).await {
        Ok(db) => db,
        Err(err) => {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = db::migrate(&db).await {
        error!("could not migrate the database: {err}");
        return ExitCode::FAILURE;
    }
    let listener = match TcpListener::bind(config.listen).await {
        Ok(listener) => listener,
        Err(err) => {
            error!("could not listen on {}: {err}", config.listen);
            return ExitCode::FAILURE;
        }
    };
    // Log the bound address, which differs from `listen` when that asks for port 0.
    match listener.local_addr() {
        Ok(addr) => info!(%addr, "listening"),
        Err(err) => {
            error!("could not read the address listened on: {err}");
            return ExitCode::FAILURE;
        }
    }

    let shutdown = async {
        shutdown.await;
        info!("shutting down");
    };
    let served = nama_hub::serve(listener, db.clone(), shutdown).await;
    db.close().await;
    if let Err(err) = served {
        error!("hub failed: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Logs to stdout at `info`, or at whatever `RUST_LOG` asks for, and, only if
/// [`OTLP_ENDPOINT`] is set, exports traces and logs there too. Returns what exports, to flush at
/// shutdown.
fn init_tracing() -> Option<Exporting> {
    // The Hub's own spans are for exporting: on stdout they'd prefix each line logged in one.
    let not_own_span = filter_fn(|metadata| {
        !(metadata.is_span() && metadata.target().starts_with(env!("CARGO_CRATE_NAME")))
    });
    let stdout = tracing_subscriber::fmt::layer()
        .with_ansi(io::stdout().is_terminal())
        .with_filter(level_filter().and(not_own_span));
    // Empty counts as unset, as the OpenTelemetry spec says of every `OTEL_*` variable.
    let endpoint = env::var_os(OTLP_ENDPOINT).filter(|endpoint| !endpoint.is_empty());
    let exporting = endpoint.map(|_| Exporting::start());
    let exported = match &exporting {
        Some(Ok(exporting)) => Some(exporting.layer()),
        Some(Err(_)) | None => None,
    };
    tracing_subscriber::registry()
        .with(stdout)
        .with(exported)
        .init();
    match exporting? {
        Ok(exporting) => {
            warn_if_https();
            Some(exporting)
        }
        Err(err) => {
            error!("could not start exporting Diagnostics, so they stay on stdout: {err}");
            None
        }
    }
}

/// The traces and logs exported over OTLP, each through a batch processor whose queue drops what
/// doesn't fit rather than growing. Every `OTEL_*` variable configures them, such as the
/// sampler, which keeps every trace unless `OTEL_TRACES_SAMPLER` says otherwise.
struct Exporting {
    traces: SdkTracerProvider,
    logs: SdkLoggerProvider,
}

impl Exporting {
    fn start() -> Result<Self, ExporterBuildError> {
        let traces = SdkTracerProvider::builder()
            .with_batch_exporter(SpanExporter::builder().with_http().build()?)
            .build();
        let logs = SdkLoggerProvider::builder()
            .with_batch_exporter(LogExporter::builder().with_http().build()?)
            .build();
        Ok(Self { traces, logs })
    }

    /// Exports the spans and lines `RUST_LOG`, or else `info`, keeps.
    fn layer<S>(&self) -> impl Layer<S> + use<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        // Lines logged in a span would join it as span events, and a line may quote what a span
        // mustn't carry (ADR 0054): they go out as logs only.
        #[expect(
            clippy::redundant_closure_for_method_calls,
            reason = "`Metadata::is_span` itself doesn't meet `filter_fn`'s lifetime bounds"
        )]
        let spans_only = filter_fn(|metadata| metadata.is_span());
        let spans = tracing_opentelemetry::layer()
            .with_tracer(self.traces.tracer(env!("CARGO_PKG_NAME")))
            .with_filter(level_filter().and(spans_only));
        let logs = OpenTelemetryTracingBridge::new(&self.logs).with_filter(level_filter());
        spans.and_then(logs)
    }

    /// Exports what is still queued, then stops.
    fn shutdown(self) {
        if let Err(err) = self.traces.shutdown() {
            error!("could not flush the last traces: {err}");
        }
        if let Err(err) = self.logs.shutdown() {
            error!("could not flush the last logs: {err}");
        }
    }
}

/// `info`, or whatever `RUST_LOG` asks for.
fn level_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Warns that an HTTPS [`OTLP_ENDPOINT`] gets nothing, since the exporter speaks only plain HTTP.
fn warn_if_https() {
    // ponytail: no TLS, as on the Server; turn on opentelemetry-otlp's `reqwest-rustls` once the
    // Hub exports straight to an HTTPS endpoint.
    if env::var(OTLP_ENDPOINT).is_ok_and(|endpoint| endpoint.starts_with("https:")) {
        warn!(
            "{OTLP_ENDPOINT} is HTTPS, which the Hub can't export to: point it at an \
             OpenTelemetry Collector over plain HTTP"
        );
    }
}

/// Resolves on Ctrl-C (SIGINT), or SIGTERM on Unix.
#[cfg(unix)]
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
    })
}

#[cfg(windows)]
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    let mut ctrl_c = tokio::signal::windows::ctrl_c()?;
    Ok(async move {
        ctrl_c.recv().await;
    })
}
