//! The Nama Hub as a library, so tests can start it in process.

mod accounts;
mod api;
pub mod config;
pub mod db;
pub mod diagnostics;
pub mod dns;
mod https;
mod links;
pub mod mail;
pub mod probe;
mod recommendations;
pub mod signing;
mod sync;
pub mod titles;
mod viewing;
#[cfg(feature = "web")]
mod web;

use std::{future::Future, io, net::SocketAddr};

use sqlx::PgPool;
use tokio::net::TcpListener;

pub use api::Handles;

/// Serves the Hub API on `listener`, backed by `db`, which must be migrated already, built from
/// `handles`, until `shutdown` resolves, then lets requests in flight finish. Must be called
/// inside a Tokio runtime, which sends them.
///
/// Clients speak the Connect protocol over HTTP/1.1 or cleartext HTTP/2, with JSON or binary
/// protobuf bodies. With the `web` feature, any other `GET` gets the web app (ADR 0105).
///
/// # Errors
///
/// The returned future fails if accepting connections on `listener` fails.
pub fn serve(
    listener: TcpListener,
    db: PgPool,
    handles: Handles,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> impl Future<Output = io::Result<()>> + Send + 'static {
    let app = api::router(db, handles);
    #[cfg(feature = "web")]
    let app = app.layer(axum::middleware::from_fn(web::serve));
    let app = app.into_make_service_with_connect_info::<SocketAddr>();
    async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}

/// Builds every title's vector from Wikidata's facts and the pooled counts, replacing the previous
/// training run's at once, and returns how many titles have one (ADR 0095). Only `train-vectors`
/// and the offline evaluation call it, never the serving process (ADR 0096).
///
/// # Errors
///
/// If a query fails. The previous run's vectors stay.
pub async fn train_vectors(db: &PgPool) -> Result<usize, sqlx::Error> {
    // Here, not in `titles`, which sits below `viewing` and can't read its counts (ADR 0099).
    titles::train(db, &viewing::pooled_counts(db).await?).await
}
