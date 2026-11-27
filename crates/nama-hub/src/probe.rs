//! Whether a Linked Server's public port takes a connection from outside (ADR 0057): a call out to
//! another network, a true external, so a trait with the TCP adapter here and a fake in the tests
//! (ADR 0099).

use std::{future::Future, net::SocketAddr, pin::Pin, time::Duration};

use tokio::net::TcpStream;

/// How long the Hub waits for a Server's public port to take a connection.
const REACH_WITHIN: Duration = Duration::from_secs(5);

/// A probe on its way to its answer.
pub type Probing<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/// Something that tries a Server's public port.
pub trait Probe: Send + Sync {
    /// Whether `public` takes a connection.
    fn reaches(&self, public: SocketAddr) -> Probing<'_>;
}

/// Tries a port with a TCP connection within `REACH_WITHIN`, and sends nothing on it.
// ponytail: a connection shows only that something listens there; check that it is this Server,
// over TLS to its name (#1830), in this adapter rather than beside it.
#[derive(Debug)]
pub struct Tcp;

impl Probe for Tcp {
    fn reaches(&self, public: SocketAddr) -> Probing<'_> {
        Box::pin(async move {
            let connected = tokio::time::timeout(REACH_WITHIN, TcpStream::connect(public)).await;
            connected.is_ok_and(|connected| connected.is_ok())
        })
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
