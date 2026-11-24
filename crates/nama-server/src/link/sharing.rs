//! Where the Server's own shared Diagnostics and Usage Analytics leave it: the Hub, through the
//! Server's Link (ADR 0054, ADR 0105).

use std::{fmt, sync::Arc};

use axum::{
    body::Bytes,
    http::{Request, Response},
};
use opentelemetry_http::{HttpClient, HttpError};
use tokio::{
    runtime::Handle,
    sync::{OwnedRwLockReadGuard, RwLock},
};

use super::Linking;
use crate::diagnostics::Signal;

/// The HTTP client an OTLP exporter of shared Diagnostics posts through: it sends each batch to
/// the Hub through the Server's Link in place of posting it, and drops it while the Server doesn't
/// serve.
#[derive(Clone)]
pub struct ShareWithHub {
    signal: Signal,
    /// The Link, and the Server's runtime it calls out on, since the exporter posts from a thread
    /// of its own. Set while the Server serves: the exporters that post through this start before
    /// the Server and stop after the database closes, which the Link would keep open. A share holds
    /// it read until it ends, so emptying it waits for those in flight.
    serving: Arc<RwLock<Option<(Linking, Handle)>>>,
}

/// The client for the exporter of traces.
impl Default for ShareWithHub {
    fn default() -> Self {
        Self {
            signal: Signal::Traces,
            serving: Arc::default(),
        }
    }
}

impl ShareWithHub {
    /// This client for the exporter of `signal`, sharing through the same Link.
    #[must_use]
    pub fn for_signal(&self, signal: Signal) -> Self {
        Self {
            signal,
            ..self.clone()
        }
    }

    /// Shares through `linking`, on the runtime this is called on, from now on, or drops batches
    /// for `None`, such as once the Server stops serving, once the shares in flight have ended.
    pub(crate) async fn serve(&self, linking: Option<Linking>) {
        *self.serving.write().await = linking.map(|linking| (linking, Handle::current()));
    }
}

impl fmt::Debug for ShareWithHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareWithHub")
            .field("signal", &self.signal)
            .finish_non_exhaustive()
    }
}

#[connectrpc::async_trait]
impl HttpClient for ShareWithHub {
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        let held = Arc::clone(&self.serving).read_owned().await;
        if let Ok(serving) = OwnedRwLockReadGuard::try_map(held, Option::as_ref) {
            let (signal, batch) = (self.signal, request.into_body());
            let runtime = serving.1.clone();
            runtime
                .spawn(async move { serving.0.share_diagnostics(signal, batch).await })
                .await?;
        }
        Ok(Response::new(Bytes::new()))
    }
}
