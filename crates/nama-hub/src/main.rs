//! The `nama-hub` binary.

use std::{
    env,
    future::Future,
    io::{self, IsTerminal},
    process::ExitCode,
    sync::Arc,
};

use clap::{Parser, Subcommand};
use nama_hub::{
    config::Config,
    db,
    diagnostics::{Backend, Otlp, OtlpError},
    dns::{Cloudflare, DnsError, ServerNames},
    mail::{LogMailer, Mail, MailError, Mailer, Resend},
    signing::Keys,
    titles,
};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{ExporterBuildError, LogExporter, SpanExporter};
use opentelemetry_sdk::{logs::SdkLoggerProvider, trace::SdkTracerProvider};
use sqlx::PgPool;
use tokio::{io::BufReader, net::TcpListener};
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
                  PostgreSQL database as a postgres:// URL, NAMA_HUB_LISTEN, the address to \
                  listen on (0.0.0.0:7981 by default), NAMA_HUB_TRUSTED_PROXIES, the reverse \
                  proxies whose forwarded headers it believes, such as [\"10.0.0.0/8\"], \
                  NAMA_HUB_SIGNING_KEYS, the Ed25519 private keys it signs with, as PEM blocks \
                  one after another, and, to email Sign-in Links, NAMA_HUB_RESEND_API_KEY, \
                  NAMA_HUB_MAIL_FROM, the address mail comes from, and NAMA_HUB_PUBLIC_URL, the \
                  NAMA_HUB_DIAGNOSTICS_HEADERS, the headers sent to it, as key=value pairs."
)]
struct Args {
    /// Serves the Hub when none is given.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Replaces the Hub's films and series with those in Wikidata's JSON dump, decompressed on
    /// stdin. Start it weekly from the host's scheduler, as its own process.
    ImportWikidata,
    /// Builds every title's vector from the facts the last import left and the pooled counts,
    /// replacing the previous run's vectors at once. Start it after each import, as its own
    /// process.
    TrainVectors,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let exporting = init_tracing();
    let exit = match args.command {
        None => run().await,
        Some(Command::ImportWikidata) => import_wikidata().await,
        Some(Command::TrainVectors) => train_vectors().await,
    };
    if let Some(exporting) = exporting {
        exporting.shutdown();
    }
    exit
}

/// Runs the Hub until it is told to stop.
#[expect(
    clippy::too_many_lines,
    reason = "each startup step logs its own failure and returns, so it reads straight down"
)]
async fn run() -> ExitCode {
    // Listen before announcing startup, so a Ctrl-C right after the log line isn't missed.
    let shutdown = match shutdown_signal() {
        Ok(shutdown) => shutdown,
        Err(err) => {
            error!("could not listen for shutdown signals: {err}");
            return ExitCode::FAILURE;
        }
    };
    let Some((config, db)) = load_config_and_connect().await else {
        return ExitCode::FAILURE;
    };
    info!(listen = %config.listen, "loaded config");
    let keys = match Keys::from_pems(&config.signing_keys) {
        Ok(keys) => keys,
        Err(err) => {
            error!("could not read the signing keys: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mail = match mail(&config) {
        Ok(mail) => mail,
        Err(err) => {
            error!("could not set up mail: {err}");
            return ExitCode::FAILURE;
        }
    };
    let names = match server_names(&config) {
        Ok(names) => names,
        Err(err) => {
            error!("could not set up the DNS provider: {err}");
            return ExitCode::FAILURE;
        }
    };
    let diagnostics = match diagnostics(&config) {
        Ok(diagnostics) => diagnostics,
        Err(err) => {
            error!("could not set up the Diagnostics backend: {err}");
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
    let served = nama_hub::serve(
        listener,
        db.clone(),
        keys,
        &config.trusted_proxies,
        mail,
        names,
        diagnostics,
        config.public_url.as_ref(),
        shutdown,
    )
    .await;
    db.close().await;
    if let Err(err) = served {
        error!("hub failed: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Mail to the log if `config` says so, or through Resend if it has its API key, which
/// [`Config::load`] has checked comes with the sender and the public URL.
fn mail(config: &Config) -> Result<Option<Mail>, MailError> {
    let Some(public_url) = &config.public_url else {
        return Ok(None);
    };
    let mailer: Arc<dyn Mailer> = if config.log_mail {
        Arc::new(LogMailer)
    } else if let (Some(api_key), Some(from)) = (&config.resend_api_key, &config.mail_from) {
        Arc::new(Resend::new(api_key.clone(), from.clone())?)
    } else {
        return Ok(None);
    };
    Ok(Some(Mail {
        mailer,
        public_url: public_url.to_string(),
    }))
}

/// The zone Linked Servers are named in, through Cloudflare, if `config` has its API token, which
/// [`Config::load`] has checked comes with the zone.
fn server_names(config: &Config) -> Result<Option<ServerNames>, DnsError> {
    let (Some(api_token), Some(zone_id), Some(zone)) = (
        &config.cloudflare_api_token,
        &config.cloudflare_zone_id,
        &config.server_names_zone,
    ) else {
        return Ok(None);
    };
    Ok(Some(ServerNames {
        zone: zone.to_ascii_lowercase(),
        records: Arc::new(Cloudflare::new(api_token.clone(), zone_id)?),
    }))
}

/// The backend what Servers share goes to, if `config` names one.
fn diagnostics(config: &Config) -> Result<Option<Arc<dyn Backend>>, OtlpError> {
    let Some(endpoint) = &config.diagnostics_endpoint else {
        return Ok(None);
    };
    Ok(Some(Arc::new(Otlp::new(
        endpoint,
        &config.diagnostics_headers,
    )?)))
}

/// Imports Wikidata's dump from stdin, leaving the titles as they were if it fails.
async fn import_wikidata() -> ExitCode {
    let Some((_, db)) = load_config_and_connect().await else {
        return ExitCode::FAILURE;
    };
    // Lines run to hundreds of KiB, and each read on stdin is a hop to a blocking thread.
    let dump = BufReader::with_capacity(1 << 20, tokio::io::stdin());
    let imported = titles::import(&db, dump).await;
    db.close().await;
    match imported {
        Ok(titles) => {
            info!(titles, "imported Wikidata's films and series");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!("could not import Wikidata's dump: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Builds the title vectors, leaving the previous run's if it fails.
async fn train_vectors() -> ExitCode {
    let Some((_, db)) = load_config_and_connect().await else {
        return ExitCode::FAILURE;
    };
    let trained = nama_hub::train_vectors(&db).await;
    db.close().await;
    match trained {
        Ok(vectors) => {
            info!(vectors, "trained the title vectors");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!("could not train the title vectors: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Loads the config and connects to its database, logging why it couldn't.
async fn load_config_and_connect() -> Option<(Config, PgPool)> {
    let config = Config::load()
        .inspect_err(|err| error!("could not load config: {err}"))
        .ok()?;
    let db = db::connect(&config.database_url)
        .await
        .inspect_err(|err| error!("{err}"))
        .ok()?;
    Some((config, db))
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
        // reqwest's `rustls-no-provider` (Cargo.toml) panics building the exporters' Client
        // without a process default. One already installed is fine.
        let _: Result<(), _> = rustls::crypto::ring::default_provider().install_default();
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
