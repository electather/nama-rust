//! An in-process Kernel for a Plugin's native tests (ADR 0065): [`with`] installs one for a call
//! to the Plugin, which then reads its settings from it and has its HTTP requests answered by the
//! test, which reads back what the Plugin sent. Unstable until the Kernel's functions can be
//! scripted too; until then they refuse, as they always have off wasm32. The crate's README shows
//! a test in its section 14.

use std::{cell::RefCell, collections::HashMap, fmt};

use ::extism_pdk::{Error, HttpRequest};

use crate::Response;

/// Answers a Plugin's HTTP request, given with its body (empty for none); an `Err` is a host
/// that can't be reached.
type Handler = Box<dyn FnMut(&HttpRequest, &[u8]) -> Result<Response, Error>>;

/// What the Kernel gives a Plugin during a test: its settings, and answers to its HTTP requests.
#[derive(Default)]
pub struct Kernel {
    settings: HashMap<String, String>,
    http: Option<Handler>,
    sent: Vec<Sent>,
}

/// An HTTP request a Plugin sent.
#[derive(Debug, Clone, PartialEq)]
pub struct Sent {
    /// Its URL, method and headers.
    pub request: HttpRequest,
    /// Its body, empty if it had none.
    pub body: Vec<u8>,
}

impl Kernel {
    /// A Kernel with no setting set, answering no request.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The Kernel, with the setting `key` holding `value` as the Server's config would.
    #[must_use]
    pub fn setting(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_owned(), value.to_owned());
        self
    }

    /// The Kernel, answering each HTTP request with `handler`, which is given its body (empty for
    /// none). An `Err` reaches the Plugin as a host that can't be reached does: the request fails
    /// with the Plugin's own words for that.
    #[must_use]
    pub fn http(
        mut self,
        handler: impl FnMut(&HttpRequest, &[u8]) -> Result<Response, Error> + 'static,
    ) -> Self {
        self.http = Some(Box::new(handler));
        self
    }

    /// The HTTP requests the Plugin sent, in order.
    #[must_use]
    pub fn sent(&self) -> &[Sent] {
        &self.sent
    }
}

impl fmt::Debug for Kernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kernel")
            .field("settings", &self.settings)
            .field("sent", &self.sent)
            .finish_non_exhaustive()
    }
}

thread_local! {
    static INSTALLED: RefCell<Option<Kernel>> = const { RefCell::new(None) };
}

/// Calls `call`, such as a Plugin's `fulfill`, with `kernel` as the Kernel, on this thread alone.
/// `kernel` then holds what the Plugin sent during it.
///
/// # Panics
///
/// When `call` panics, as when the Plugin sends a request `kernel` has no answer for.
#[expect(
    clippy::expect_used,
    reason = "`call` can't take the installed Kernel out of its slot"
)]
pub fn with<R>(kernel: &mut Kernel, call: impl FnOnce() -> R) -> R {
    let outer = INSTALLED.replace(Some(std::mem::take(kernel)));
    let answered = call();
    *kernel = INSTALLED
        .replace(outer)
        .expect("the Kernel stays installed through the call");
    answered
}

/// The setting `key` the installed Kernel holds.
pub(crate) fn setting(key: &str) -> Option<String> {
    installed(|kernel| kernel.settings.get(key).cloned())
}

/// The installed Kernel's answer to `request` with `body`, recording both.
#[expect(
    clippy::panic,
    reason = "an unscripted request is the test's bug, which an error the Plugin may retry or swallow would hide"
)]
pub(crate) fn http_request(request: &HttpRequest, body: &[u8]) -> Result<Response, Error> {
    installed(|kernel| {
        kernel.sent.push(Sent {
            request: request.clone(),
            body: body.to_vec(),
        });
        let Some(handler) = &mut kernel.http else {
            let method = request.method.as_deref().unwrap_or("GET");
            panic!(
                "the test scripted no HTTP answers, yet the Plugin sent {method} {}",
                request.url
            );
        };
        handler(request, body)
    })
}

#[expect(
    clippy::expect_used,
    reason = "a Plugin called outside `with` is the test's bug"
)]
fn installed<R>(read: impl FnOnce(&mut Kernel) -> R) -> R {
    INSTALLED.with_borrow_mut(|kernel| {
        read(
            kernel
                .as_mut()
                .expect("call the Plugin inside nama_pdk::testing::with, which gives it a Kernel"),
        )
    })
}

#[cfg(test)]
mod tests {
    use ::extism_pdk::{Error, HttpRequest};

    use super::{Kernel, with};
    use crate::Response;

    fn get(url: &str) -> Result<Response, Error> {
        crate::http_request(&HttpRequest::new(url), None::<String>, "nothing answered")
    }

    #[test]
    fn the_handlers_answer_reaches_the_plugin() {
        let mut kernel = Kernel::new().http(|_request, _body| Ok(Response::new(201, "made")));

        let answered = with(&mut kernel, || get("http://service.test/things")).unwrap();

        assert_eq!(
            (answered.status_code(), answered.body()),
            (201, &b"made"[..])
        );
    }

    #[test]
    fn requests_are_recorded_in_the_order_sent_with_their_bodies() {
        let mut kernel = Kernel::new().http(|_request, _body| Ok(Response::new(204, "")));

        with(&mut kernel, || {
            let post = HttpRequest::new("http://service.test/a").with_method("POST");
            crate::http_request(&post, Some("first"), "nothing answered")?;
            get("http://service.test/b")
        })
        .unwrap();

        let sent: Vec<_> = (kernel.sent().iter())
            .map(|sent| {
                let method = sent.request.method.as_deref();
                (method, sent.request.url.as_str(), sent.body.as_slice())
            })
            .collect();
        assert_eq!(
            sent,
            [
                (Some("POST"), "http://service.test/a", &b"first"[..]),
                (None, "http://service.test/b", &b""[..]),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "the Plugin sent GET http://service.test/unscripted")]
    fn an_unscripted_request_panics() {
        let _unanswered = with(&mut Kernel::new(), || get("http://service.test/unscripted"));
    }

    #[test]
    fn a_host_that_cant_be_reached_fails_with_the_plugins_words() {
        let mut kernel = Kernel::new().http(|_request, _body| Err(Error::msg("refused")));

        let failed = with(&mut kernel, || get("http://service.test")).unwrap_err();

        assert_eq!(failed.to_string(), "nothing answered");
    }

    #[test]
    fn settings_read_back_and_an_unset_one_is_none() {
        let mut kernel = Kernel::new().setting("url", "http://service.test");

        let read = with(&mut kernel, || {
            (
                crate::setting("url").unwrap(),
                crate::setting("api_key").unwrap(),
            )
        });

        assert_eq!(read, (Some("http://service.test".to_owned()), None));
    }
}
