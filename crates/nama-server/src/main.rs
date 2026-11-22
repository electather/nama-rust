//! The `nama-server` binary.

use std::{
    env,
    future::Future,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};

use clap::{Parser, Subcommand};
use nama_plugin_api::{
    diagnostics_sharer::Signal,
    points::{ServerStarted, ServerStartedEvent},
};
use nama_server::{
    backup,
    config::{self, Config},
    db::{self, Database},
    diagnostics,
    ffmpeg::Ffmpeg,
    plugin_settings::PluginSettings,
    plugins::{Registry, ShareThroughPlugins},
    users::{SetupCode, Users},
};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{ExporterBuildError, LogExporter, SpanExporter, WithHttpConfig as _};
use opentelemetry_sdk::{
    logs::SdkLoggerProvider,
    trace::{BatchSpanProcessor, SdkTracerProvider},
};
use tokio::{net::TcpListener, runtime::Handle, task};
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
#[command(version, about)]
struct Args {
    /// TOML config file. Defaults to `nama.toml` in the working directory, if it exists.
    #[arg(long, env = config::FILE_ENV, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

/// What to do instead of serving.
#[derive(Debug, Subcommand)]
enum Command {
    /// Writes a backup of the Server to a `.tar` file. Safe while it serves.
    Backup {
        /// Where to write the backup, replacing any file there.
        path: PathBuf,
    },
    /// Replaces the Server's database, config file and Custom Images with a backup's, keeping
    /// each it replaces beside it with `.before-restore` added to its name. Stop the Server first.
    Restore {
        /// The backup to restore.
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    // The traces exporter's client; the logs exporter's is made from it.
    let sharing = ShareThroughPlugins::new(Signal::Traces, Handle::current());
    let exporting = init_tracing(&sharing);
    let to_admin = exporting.as_ref().is_some_and(Exporting::to_admin);
    let exit = match args.command {
        None => run(args.config.as_deref(), to_admin, &sharing).await,
        Some(command) => back_up_or_restore(args.config, command).await,
    };
    if let Some(exporting) = exporting {
        exporting.shutdown();
    }
    exit
}

/// Runs the Server until it is told to stop. `exporting` says whether Diagnostics are exported to
/// the admin's endpoint; what they share goes out through `sharing`.
async fn run(
    config_file: Option<&Path>,
    exporting: bool,
    sharing: &ShareThroughPlugins,
) -> ExitCode {
    let Some(mut config) = load_config(config_file) else {
        return ExitCode::FAILURE;
    };
    if exporting {
        config.otlp_endpoint = env::var(OTLP_ENDPOINT).ok();
    }

    // Listen before announcing startup, so a Ctrl-C right after the log line isn't missed.
    let shutdown = match shutdown_signal() {
        Ok(shutdown) => shutdown,
        Err(err) => {
            error!("could not listen for shutdown signals: {err}");
            return ExitCode::FAILURE;
        }
    };

    info!(
        name = %config.name,
        listen = %config.listen,
        data_dir = %config.data_dir.display(),
        "loaded config"
    );

    let Some(db) = open_database(&config).await else {
        return ExitCode::FAILURE;
    };
    let Some(plugins) = start_plugins(&config, &db).await else {
        close(db).await;
        return ExitCode::FAILURE;
    };
    let setup_code = match setup_code(&db, &plugins).await {
        Ok(setup_code) => setup_code,
        Err(err) => {
            error!("could not prepare the Setup Code: {err}");
            close(db).await;
            return ExitCode::FAILURE;
        }
    };

    let ffmpeg = find_ffmpeg(&config).await;
    let listener = match TcpListener::bind(config.listen).await {
        Ok(listener) => listener,
        Err(err) => {
            error!("could not listen on {}: {err}", config.listen);
            close(db).await;
            return ExitCode::FAILURE;
        }
    };
    // Log the bound address, which differs from `listen` when that asks for port 0.
    match listener.local_addr() {
        Ok(addr) => info!(%addr, "listening"),
        Err(err) => {
            error!("could not read the address listened on: {err}");
            close(db).await;
            return ExitCode::FAILURE;
        }
    }

    let points = plugins.extension_points();
    points.notify::<ServerStarted>(ServerStartedEvent {
        name: config.name.clone(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    });

    let shutdown = async {
        shutdown.await;
        info!("shutting down");
    };
    sharing.serve(Some(points)).await;
    let served = nama_server::serve(
        listener,
        &config,
        Arc::clone(&db),
        plugins,
        setup_code,
        ffmpeg,
        shutdown,
    )
    .await;
    sharing.serve(None).await;
    // Serving has finished, so this is the last handle and the database can close cleanly.
    close(db).await;
    if let Err(err) = served {
        error!("server failed: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Writes or restores a backup, as `command` says, of the Server `config_file` configures.
async fn back_up_or_restore(config_file: Option<PathBuf>, command: Command) -> ExitCode {
    let Some(config) = load_config(config_file.as_deref()) else {
        return ExitCode::FAILURE;
    };
    let (done, path, outcome) = match command {
        Command::Backup { path } => (
            backup::write(&config.data_dir, &config.file, &path).await,
            path,
            "wrote the backup",
        ),
        Command::Restore { path } => {
            let restored = backup::restore(&config.data_dir, &config.file, &path).await;
            if restored.is_ok() {
                warn_if_data_dir_moved(&config.data_dir, &config.file);
            }
            (
                restored,
                path,
                "restored the backup; what it replaced is kept beside it as `.before-restore`",
            )
        }
    };
    match done {
        Ok(()) => {
            info!(path = %path.display(), "{outcome}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!(path = %path.display(), "{err}");
            ExitCode::FAILURE
        }
    }
}

/// Warns when the config file a restore put back names another data directory than the one it
/// restored into, where the next start wouldn't find what it restored.
fn warn_if_data_dir_moved(restored_into: &Path, config_file: &Path) {
    if let Ok((restored, _)) = Config::load(Some(config_file))
        && restored.data_dir != restored_into
    {
        warn!(
            restored_into = %restored_into.display(),
            config_names = %restored.data_dir.display(),
            "the restored config file names another data directory: set `data_dir` in it, or \
             `NAMA_DATA_DIR`, to the one restored into"
        );
    }
}

/// Loads the config, warning about each key in the file it ignores. Returns `None`, having logged
/// why, if it can't.
fn load_config(file: Option<&Path>) -> Option<Config> {
    let (mut config, unknown_keys) = match Config::load(file) {
        Ok(loaded) => loaded,
        Err(err) => {
            error!("could not load config: {err}");
            return None;
        }
    };
    if let Some(file) = file {
        file.clone_into(&mut config.file);
    }
    // Only an unknown key's name is logged: its value may be a secret written in the wrong place.
    for key in unknown_keys {
        warn!(
            key,
            file = %config.file.display(),
            "ignoring a config key the Server doesn't know; the file's `#:schema` line lets an editor check it"
        );
    }
    Some(config)
}

/// Opens the database in the data directory. Returns `None`, having logged why, if it can't.
async fn open_database(config: &Config) -> Option<Arc<Database>> {
    let db = match Database::open(&config.data_dir).await {
        Ok(db) => db,
        Err(err) => {
            error!(
                "could not open the database in {}: {err}",
                config.data_dir.display()
            );
            return None;
        }
    };
    info!(
        path = %config.data_dir.join(db::FILE_NAME).display(),
        migrations_applied = ?db.migrations_applied(),
        "opened database"
    );
    Some(Arc::new(db))
}

/// Starts the Built-in Plugins and the Installed Plugins in the data directory. Returns `None`,
/// having logged why, if they can't start.
async fn start_plugins(config: &Config, db: &Arc<Database>) -> Option<PluginSettings> {
    PluginSettings::start(Arc::clone(db), Registry::with_builtins(), config)
        .await
        .inspect_err(|err| error!("{err}"))
        .ok()
}

/// A new Setup Code, logged, while the Server has no Users, so whoever can read the log can create
/// the first Admin.
async fn setup_code(
    db: &Arc<Database>,
    plugins: &PluginSettings,
) -> Result<Option<SetupCode>, Box<dyn std::error::Error>> {
    let users = Users::new(Arc::clone(db), plugins.extension_points());
    if users.any().await? {
        return Ok(None);
    }
    let code = SetupCode::generate()?;
    info!(setup_code = %code, "this Server has no Users yet: create the first Admin with this Setup Code");
    Ok(Some(code))
}

/// The ffmpeg and ffprobe at the paths `config` sets or on `PATH`, logging what was found, or why
/// the Server can only Direct Play without them.
async fn find_ffmpeg(config: &Config) -> Ffmpeg {
    let config = config.clone();
    let path = env::var_os("PATH").unwrap_or_default();
    let ffmpeg = task::spawn_blocking(move || Ffmpeg::find(&config, &path))
        .await
        .inspect_err(|err| error!("looking for ffmpeg failed: {err}"))
        .unwrap_or_default();
    if let Some(reason) = ffmpeg.direct_play_only_because() {
        warn!("{reason}, so this Server can only Direct Play, not Remux or Transcode");
    } else {
        info!(?ffmpeg, "found ffmpeg and ffprobe");
    }
    ffmpeg
}

/// Closes the database, which must hold the last handle to it.
async fn close(db: Arc<Database>) {
    if let Some(db) = Arc::into_inner(db) {
        db.close().await;
    } else {
        error!("the database was still in use at shutdown; it was not closed cleanly");
    }
}

/// Logs to stdout at `info`, or at whatever `RUST_LOG` asks for, and, only if
/// [`OTLP_ENDPOINT`] is set, exports traces and logs there too. Shares what the admin shares
/// through `sharing`. Returns what exports, to flush at shutdown.
fn init_tracing(sharing: &ShareThroughPlugins) -> Option<Exporting> {
    // Extism logs an Installed Plugin's error or panic, and the lines it writes through Extism's
    // own log functions, as they are, which may quote a secret setting; the Server logs both
    // itself with secrets blanked (ADR 0061).
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,extism::plugin=off,extism::pdk=off"));
    // The Server's own spans are for exporting: on stdout they'd prefix each line logged in one.
    // Usage Analytics are for sharing only.
    let not_own_span = filter_fn(|metadata| {
        !(metadata.is_span() && metadata.target().starts_with(env!("CARGO_CRATE_NAME"))
            || metadata.target() == diagnostics::USAGE_TARGET)
    });
    // One filter for both, so the Plugins page keeps the Plugin Log lines stdout shows (ADR 0061).
    let stdout = tracing_subscriber::fmt::layer()
        .with_ansi(io::stdout().is_terminal())
        .and_then(diagnostics::plugin_lines_layer())
        .with_filter(filter.and(not_own_span));
    // Empty counts as unset, as the OpenTelemetry spec says of every `OTEL_*` variable.
    let to_admin = env::var_os(OTLP_ENDPOINT).is_some_and(|endpoint| !endpoint.is_empty());
    let exporting = Exporting::start(to_admin, sharing);
    tracing_subscriber::registry()
        .with(stdout)
        .with(exporting.as_ref().ok().map(Exporting::layer))
        .init();
    match exporting {
        Ok(exporting) => {
            if to_admin {
                warn_if_https();
            }
            Some(exporting)
        }
        Err(err) => {
            error!("could not start exporting Diagnostics, so they stay on stdout: {err}");
            None
        }
    }
}

/// The traces and logs exported over OTLP, each through a batch processor whose queue drops what
/// doesn't fit rather than growing: to the admin's endpoint, if they set one, and what they share
/// to Nama's developers. Every `OTEL_*` variable configures them, such as the sampler, which
/// keeps every trace unless `OTEL_TRACES_SAMPLER` says otherwise.
struct Exporting {
    /// Both the admin's spans and the shared ones: one provider, since a span records once.
    traces: SdkTracerProvider,
    /// The admin's, while they set an endpoint.
    logs: Option<SdkLoggerProvider>,
    shared_logs: SdkLoggerProvider,
}

impl Exporting {
    /// Exports to [`OTLP_ENDPOINT`] if `to_admin`, and what the admin shares through `sharing`.
    fn start(to_admin: bool, sharing: &ShareThroughPlugins) -> Result<Self, ExporterBuildError> {
        let shared_spans = SpanExporter::builder()
            .with_http()
            .with_http_client(sharing.clone())
            .build()?;
        let mut traces =
            SdkTracerProvider::builder().with_span_processor(diagnostics::SharedSpans::new(
                BatchSpanProcessor::builder(shared_spans).build(),
                &diagnostics::SHARING,
            ));
        let shared_logs = SdkLoggerProvider::builder()
            .with_resource(diagnostics::shared_resource())
            .with_batch_exporter(
                LogExporter::builder()
                    .with_http()
                    .with_http_client(sharing.for_signal(Signal::Logs))
                    .build()?,
            )
            .build();
        let logs = if to_admin {
            traces = traces.with_batch_exporter(SpanExporter::builder().with_http().build()?);
            Some(
                SdkLoggerProvider::builder()
                    .with_batch_exporter(LogExporter::builder().with_http().build()?)
                    .build(),
            )
        } else {
            None
        };
        Ok(Self {
            traces: traces.build(),
            logs,
            shared_logs,
        })
    }

    /// Whether this exports to the admin's endpoint.
    fn to_admin(&self) -> bool {
        self.logs.is_some()
    }

    fn layer<S>(&self) -> impl Layer<S> + use<S>
    where
        S: Subscriber + for<'span> LookupSpan<'span>,
    {
        let spans = (self.to_admin()).then(|| diagnostics::layer(&self.traces));
        // Without the admin's endpoint, spans record only while they are shared.
        let shared_spans = (!self.to_admin())
            .then(|| diagnostics::sharing_layer(&self.traces, &diagnostics::SHARING));
        let logs = (self.logs.as_ref()).map(|logs| {
            OpenTelemetryTracingBridge::new(logs).with_filter(diagnostics::exported_filter())
        });
        let shared_logs = OpenTelemetryTracingBridge::new(&self.shared_logs)
            .with_filter(diagnostics::shared_logs_filter(&diagnostics::SHARING));
        shared_logs
            .and_then(logs)
            .and_then(spans)
            .and_then(shared_spans)
    }

    /// Exports what is still queued, then stops.
    fn shutdown(self) {
        if let Err(err) = self.traces.shutdown() {
            error!("could not flush the last traces: {err}");
        }
        for logs in self.logs.iter().chain([&self.shared_logs]) {
            if let Err(err) = logs.shutdown() {
                error!("could not flush the last logs: {err}");
            }
        }
    }
}

/// Warns that an HTTPS [`OTLP_ENDPOINT`] gets nothing, since the exporter speaks only plain HTTP.
fn warn_if_https() {
    // ponytail: no TLS, which would add a C crypto build; turn on opentelemetry-otlp's
    // `reqwest-rustls` once an admin needs to export straight to an HTTPS endpoint.
    if env::var(OTLP_ENDPOINT).is_ok_and(|endpoint| endpoint.starts_with("https:")) {
        warn!(
            "{OTLP_ENDPOINT} is HTTPS, which this Server can't export to: point it at an \
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
