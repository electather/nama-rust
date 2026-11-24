//! Where shared Diagnostics and Usage Analytics leave the Server: the enabled Plugin that provides
//! [`DiagnosticsSharer`], such as the Hub Plugin (ADR 0054, ADR 0094).

use std::{fmt, sync::Arc};

use axum::{
    body::Bytes,
    http::{Request, Response},
};
use nama_plugin_api::diagnostics_sharer::{DiagnosticsSharer, Signal};
use opentelemetry_http::{HttpClient, HttpError};
use tokio::{
    runtime::Handle,
    sync::{OwnedRwLockReadGuard, RwLock},
};
use tracing::{instrument::WithSubscriber as _, subscriber::NoSubscriber};

use super::ExtensionPoints;

/// The HTTP client an OTLP exporter of shared Diagnostics posts through: it hands each batch to
/// the enabled [`DiagnosticsSharer`] in place of posting it, and drops it while none is enabled.
#[derive(Clone)]
pub struct ShareThroughPlugins {
    signal: Signal,
    /// Set while the Server serves: the exporters that post through this start before the Plugins
    /// and stop after the database closes, which these would keep open. A share holds it read
    /// until it ends, so emptying it waits for those in flight.
    points: Arc<RwLock<Option<ExtensionPoints>>>,
    /// The Server's runtime, which the Plugins call out on: the exporter posts from a thread of its
    /// own.
    runtime: Handle,
}

impl ShareThroughPlugins {
    /// A client for the exporter of `signal`, calling Plugins on `runtime` while [`Self::serve`]
    /// has handed it their Extension Points.
    #[must_use]
    pub fn new(signal: Signal, runtime: Handle) -> Self {
        Self {
            signal,
            points: Arc::default(),
            runtime,
        }
    }

    /// This client for the exporter of `signal`, handing batches to the same Plugins.
    #[must_use]
    pub fn for_signal(&self, signal: Signal) -> Self {
        Self {
            signal,
            ..self.clone()
        }
    }

    /// Hands batches to the sharer among `points` from now on, or drops them for `None`, such as
    /// once the Server stops serving, once the shares in flight have ended.
    pub async fn serve(&self, points: Option<ExtensionPoints>) {
        *self.points.write().await = points;
    }
}

impl fmt::Debug for ShareThroughPlugins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareThroughPlugins")
            .field("signal", &self.signal)
            .finish_non_exhaustive()
    }
}

#[connectrpc::async_trait]
impl HttpClient for ShareThroughPlugins {
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        let held = Arc::clone(&self.points).read_owned().await;
        if let Ok(points) = OwnedRwLockReadGuard::try_map(held, Option::as_ref) {
            let (signal, batch) = (self.signal, request.into_body());
            self.runtime
                .spawn(async move { share_diagnostics(&points, signal, batch).await })
                .await?;
        }
        Ok(Response::new(Bytes::new()))
    }
}

/// Hands `batch`, an OTLP/HTTP protobuf export request of `signal`, to the enabled
/// [`DiagnosticsSharer`] among `points`, if any.
pub(crate) async fn share_diagnostics(points: &ExtensionPoints, signal: Signal, batch: Bytes) {
    let shared = async {
        // A failure shows on the Plugin's page, as any Provider's does, and in the Server's status.
        let answer = points
            .ask_first::<dyn DiagnosticsSharer, _, _, _, _>(|sharer| {
                let batch = batch.to_vec();
                async move { sharer.share(signal, batch).await }
            })
            .await;
        if answer.is_some_and(|answer| answer.result.is_err()) {
            crate::status::share_refused();
        }
    };
    // Traced, sharing would record spans of its own that the next batch would share, and so on
    // for as long as the Server runs.
    shared.with_subscriber(NoSubscriber::default()).await;
}
