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
    fetch_with(server, path, token, &[]).await
}

/// Like [`fetch`], also sending `headers`, such as the `range` a browser's `<video>` asks for to
/// seek.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn fetch_with(
    server: &TestServer,
    path: &str,
    token: Option<&str>,
    headers: &[(&str, &str)],
) -> Reply {
    let mut request = hyper::Request::get(path);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let empty = http_body_util::Empty::<hyper::body::Bytes>::new();
    send(server, request.body(empty).unwrap(), token).await
}

/// Sends `request` to `server` over plain HTTP/1.1, presenting `token`, if any, in the session
/// cookie, as a browser does.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) async fn send<B>(
    server: &TestServer,
    mut request: hyper::Request<B>,
    token: Option<&str>,
) -> Reply
where
    B: hyper::body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    use http_body_util::BodyExt;
    use hyper::{client::conn::http1, header::HeaderValue};
    use hyper_util::rt::TokioIo;

    let stream = tokio::net::TcpStream::connect(server.addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let headers = request.headers_mut();
    headers.insert(
        "host",
        HeaderValue::from_str(&server.addr.to_string()).unwrap(),
    );
    if let Some(token) = token {
        let cookie = format!("nama_session={token}");
        headers.insert("cookie", HeaderValue::from_str(&cookie).unwrap());
    }
    let response = sender.send_request(request).await.unwrap();
    let (parts, body) = response.into_parts();
    Reply {
        status: parts.status.as_u16(),
        headers: parts.headers,
        body: body.collect().await.unwrap().to_bytes().to_vec(),
    }
}
