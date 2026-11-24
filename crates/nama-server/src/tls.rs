//! HTTPS and plain HTTP on the one port the Server listens on (ADR 0057): a connection that opens
//! with a TLS handshake gets the certificate for the Server's names (`certificate.rs`), and any
//! other is plain HTTP, which the LAN keeps using whether or not the Server has one.

use std::{convert::Infallible, future::Future, io, net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{ConnectInfo, Request},
    serve::{IncomingStream, Listener},
};
use nama_forwarded::ServedOverTls;
use rustls::{ServerConfig, server::ResolvesServerCert};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use tokio_util::either::Either;
use tower::ServiceExt as _;

/// The first byte of a TLS record carrying a handshake, as a TLS connection's first always does
/// (RFC 8446, section 5.1). No HTTP request starts with it.
const HANDSHAKE: u8 = 0x16;

/// How long a new connection may take to send its first byte, and then its TLS handshake, before
/// it is dropped, so a client that stalls doesn't hold a task for ever.
const OPENING_WITHIN: Duration = Duration::from_secs(30);

/// How many opened connections may wait for axum to take them.
const OPENED_WAITING: usize = 64;

type Connection = Either<TcpStream, TlsStream<TcpStream>>;

/// Serves `app` on `listener`, over TLS with `certificate`'s certificate to a client that starts a
/// handshake and plain HTTP to any other, until `shutdown` resolves, then lets requests in flight
/// finish. Each request carries its peer's [`ConnectInfo`], and [`ServedOverTls`] over TLS.
///
/// # Errors
///
/// When TLS can't be set up, or `listener`'s address can't be read.
pub(crate) async fn serve(
    listener: TcpListener,
    app: Router,
    certificate: Arc<dyn ResolvesServerCert>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let opened = open(listener, certificate)?;
    let per_connection = tower::service_fn(move |incoming: IncomingStream<'_, Opened>| {
        let peer = ConnectInfo(*incoming.remote_addr());
        let over_tls = matches!(incoming.io(), Either::Right(_));
        let served = app.clone().map_request(move |mut request: Request| {
            request.extensions_mut().insert(peer);
            if over_tls {
                request.extensions_mut().insert(ServedOverTls);
            }
            request
        });
        std::future::ready(Ok::<_, Infallible>(served))
    });
    axum::serve(opened, per_connection)
        .with_graceful_shutdown(shutdown)
        .await
}

/// Connections opened, which axum takes in turn.
struct Opened {
    local: SocketAddr,
    connections: mpsc::Receiver<(Connection, SocketAddr)>,
}

impl Listener for Opened {
    type Io = Connection;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.connections.recv().await {
            Some(opened) => opened,
            // Accepting ended, which it does only once this is dropped.
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Ok(self.local)
    }
}

/// Accepts connections on `listener` until the returned [`Opened`] is dropped, opening each in a
/// task of its own, so one that stalls holds up no other.
fn open(listener: TcpListener, certificate: Arc<dyn ResolvesServerCert>) -> io::Result<Opened> {
    let local = listener.local_addr()?;
    let acceptor = TlsAcceptor::from(Arc::new(tls_config(certificate)?));
    let (opened, connections) = mpsc::channel(OPENED_WAITING);
    tokio::spawn(async move {
        loop {
            let accepted = tokio::select! {
                accepted = listener.accept() => accepted,
                () = opened.closed() => return,
            };
            let (stream, peer) = match accepted {
                Ok(accepted) => accepted,
                Err(err) => {
                    pause_after(&err).await;
                    continue;
                }
            };
            tokio::spawn(hand_over(stream, peer, acceptor.clone(), opened.clone()));
        }
    });
    Ok(Opened { local, connections })
}

/// Opens `stream` from `peer` and hands it to `opened`, unless it fails to open.
async fn hand_over(
    stream: TcpStream,
    peer: SocketAddr,
    acceptor: TlsAcceptor,
    opened: mpsc::Sender<(Connection, SocketAddr)>,
) {
    if let Some(connection) = open_one(stream, &acceptor).await {
        // Fails only once serving has stopped, which takes no more connections.
        let _: Result<(), _> = opened.send((connection, peer)).await;
    }
}

/// TLS that serves `certificate`'s certificate, offering HTTP/2 and HTTP/1.1.
fn tls_config(certificate: Arc<dyn ResolvesServerCert>) -> io::Result<ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_cert_resolver(certificate);
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

/// `stream`, through a TLS handshake if it starts one, or as it is. `None` once it closes, stalls,
/// or its handshake fails, as it does while the Server has no certificate.
async fn open_one(stream: TcpStream, acceptor: &TlsAcceptor) -> Option<Connection> {
    let opening = async move {
        let mut first = [0; 1];
        if stream.peek(&mut first).await.ok()? == 0 {
            return None;
        }
        if first[0] != HANDSHAKE {
            return Some(Either::Left(stream));
        }
        acceptor.accept(stream).await.ok().map(Either::Right)
    };
    tokio::time::timeout(OPENING_WITHIN, opening)
        .await
        .ok()
        .flatten()
}

/// Waits after accepting failed with `err`, as axum's own listener does: not after a connection
/// that failed alone, but a second after another error, such as running out of file descriptors,
/// which retrying at once would only spin on.
async fn pause_after(err: &io::Error) {
    let connection_failed = matches!(
        err.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    );
    if !connection_failed {
        tracing::error!("accepting a connection failed: {err}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
