//! Plain HTTP requests to a test Server, as a browser makes them for media.

use crate::server::TestServer;

/// A response, as [`fetch`] reads it.
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: hyper::HeaderMap,
    pub(crate) body: Vec<u8>,
}

impl Reply {
    /// The value of the header `name`, or `""` if it has none.
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
    pub(crate) fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .map_or("", |value| value.to_str().unwrap())
    }
}

/// Sends a plain HTTP/1.1 `GET` for `path` to `server`, presenting `token`, if any, in the
/// session cookie, as a browser's `<img src>` does.
pub(crate) async fn fetch(server: &TestServer, path: &str, token: Option<&str>) -> Reply {
    fetch_range(server, path, token, None).await
}

/// Like [`fetch`], asking for only the bytes in `range`, if any, as a browser's `<video>` does
/// to seek.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn fetch_range(
    server: &TestServer,
    path: &str,
    token: Option<&str>,
    range: Option<&str>,
) -> Reply {
    use http_body_util::{BodyExt, Empty};
    use hyper::{Request, body::Bytes, client::conn::http1};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(server.addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let mut request = Request::get(path).header("host", server.addr.to_string());
    if let Some(token) = token {
        request = request.header("cookie", format!("nama_session={token}"));
    }
    if let Some(range) = range {
        request = request.header("range", range);
    }
    let response = sender
        .send_request(request.body(Empty::<Bytes>::new()).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    Reply {
        status: parts.status.as_u16(),
        headers: parts.headers,
        body: body.collect().await.unwrap().to_bytes().to_vec(),
    }
}
