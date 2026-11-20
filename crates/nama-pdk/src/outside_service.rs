//! An outside service a Plugin talks to over HTTP, and the one way its refusals are worded.

use std::fmt;

use ::extism_pdk::{Error, HttpRequest, ToBytes};

use crate::Response;

/// An outside service a Plugin sends requests to, such as Discord, as its errors name it. Build
/// it as a constant: `OutsideService::new("fanart.tv").credential("API key")`.
#[derive(Debug, Clone, Copy)]
pub struct OutsideService {
    name: &'static str,
    credential: Option<&'static str>,
    reader: Option<fn(&Response) -> Option<String>>,
}

impl OutsideService {
    /// The service called `name` as the subject of a sentence, such as "Discord", "fanart.tv" or
    /// "the webhook".
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            credential: None,
            reader: None,
        }
    }

    /// The service, whose 401 means it refused `credential`, such as "API key".
    #[must_use]
    pub const fn credential(self, credential: &'static str) -> Self {
        Self {
            credential: Some(credential),
            ..self
        }
    }

    /// The service, whose own words `reader` picks from a refusal, or `None` when it gave none.
    /// It picks the service's error message field alone, never the whole body, which may hold
    /// what the request sent (ADR 0061).
    #[must_use]
    pub const fn reader(self, reader: fn(&Response) -> Option<String>) -> Self {
        Self {
            reader: Some(reader),
            ..self
        }
    }

    /// Sends `request` with `body` through [`crate::http_request`].
    ///
    /// # Errors
    ///
    /// "{name} didn't answer" when nothing answers, or the kit's own error when `request` or
    /// `body` can't be passed to the Server.
    pub fn send<'a, T: ToBytes<'a>>(
        &self,
        request: &HttpRequest,
        body: Option<T>,
    ) -> Result<Response, Error> {
        crate::http_request(request, body, &format!("{} didn't answer", self.name))
    }

    /// `response` when its status is 2xx. A Plugin matches the statuses that mean something of
    /// its own first, and hands the rest here.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] of any other status.
    pub fn accepted(&self, response: Response) -> Result<Response, Refusal> {
        if (200..300).contains(&response.status_code()) {
            Ok(response)
        } else {
            Err(self.refusal(&response))
        }
    }

    /// As [`OutsideService::accepted`], except that a 404, the service having nothing there,
    /// gives `None`.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] of any status but 2xx and 404.
    pub fn found(&self, response: Response) -> Result<Option<Response>, Refusal> {
        if response.status_code() == 404 {
            return Ok(None);
        }
        self.accepted(response).map(Some)
    }

    fn refusal(&self, response: &Response) -> Refusal {
        let status = response.status_code();
        // A `Retry-After` may also be an HTTP date, which isn't worth parsing for a wording.
        let retry_after_seconds = response
            .header("Retry-After")
            .and_then(|after| after.parse().ok());
        let name = self.name;
        let wording = match (status, self.credential) {
            (401, Some(credential)) => format!("{name} refused the {credential}"),
            (429, _) => {
                let wait = retry_after_seconds.map_or_else(
                    || "a few seconds".to_owned(),
                    |seconds| format!("{seconds} s"),
                );
                format!("{name} is limiting this Server's requests; try again in {wait}")
            }
            _ => match (self.reader)
                .and_then(|reader| reader(response))
                .filter(|words| !words.trim().is_empty())
            {
                Some(words) => format!("{name} said: {words}"),
                None => format!("{name} failed with HTTP status {status}"),
            },
        };
        Refusal {
            status,
            retry_after_seconds,
            wording,
        }
    }
}

/// Why an [`OutsideService`] refused a request, worded for an admin; it names no URL, header,
/// query, body or setting value. A Plugin reaches it in an `extism_pdk::Error` with
/// `downcast_ref` when a status means something after the fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    status: u16,
    retry_after_seconds: Option<u64>,
    wording: String,
}

impl Refusal {
    /// The response's HTTP status, such as 429.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The seconds the response's `Retry-After` said to wait, when it gave whole seconds.
    #[must_use]
    pub fn retry_after_seconds(&self) -> Option<u64> {
        self.retry_after_seconds
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.wording)
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use ::extism_pdk::{Error, HttpRequest};

    use super::{OutsideService, Refusal};
    use crate::{
        Response,
        testing::{Kernel, with},
    };

    const WEBHOOK: OutsideService = OutsideService::new("the webhook");
    const FANART: OutsideService = OutsideService::new("fanart.tv").credential("API key");
    const DISCORD: OutsideService = OutsideService::new("Discord").reader(message);

    /// Discord's own words: the `message` field of its JSON body.
    fn message(response: &Response) -> Option<String> {
        let body: std::collections::HashMap<String, String> = response.json().ok()?;
        body.get("message").cloned()
    }

    /// What `service` makes of `answer` to a request it sends.
    fn sent(service: OutsideService, answer: Response) -> Result<Response, Error> {
        let mut answer = Some(answer);
        let mut kernel = Kernel::new()
            .http(move |_request, _body| Ok(answer.take().expect("the service sent one request")));
        with(&mut kernel, || {
            let response = service.send(&HttpRequest::new("http://service.test"), None::<&str>)?;
            Ok(service.accepted(response)?)
        })
    }

    fn refused(service: OutsideService, answer: Response) -> String {
        sent(service, answer).unwrap_err().to_string()
    }

    #[test]
    fn a_service_that_doesnt_answer_is_named() {
        let mut kernel = Kernel::new().http(|_request, _body| Err(Error::msg("refused")));

        let failed = with(&mut kernel, || {
            FANART.send(&HttpRequest::new("http://service.test"), None::<&str>)
        })
        .unwrap_err();

        assert_eq!(failed.to_string(), "fanart.tv didn't answer");
    }

    #[test]
    fn a_2xx_is_accepted() {
        let accepted = sent(FANART, Response::new(204, "done")).unwrap();

        assert_eq!(accepted, Response::new(204, "done"));
    }

    #[test]
    fn a_401_names_the_credential_refused() {
        assert_eq!(
            refused(FANART, Response::new(401, "")),
            "fanart.tv refused the API key"
        );
    }

    #[test]
    fn a_401_without_a_credential_is_a_failed_status() {
        assert_eq!(
            refused(WEBHOOK, Response::new(401, "")),
            "the webhook failed with HTTP status 401"
        );
    }

    #[test]
    fn a_429_says_when_to_try_again() {
        let limited = |retry_after: Option<&str>| {
            let answer = Response::new(429, r#"{"message": "slow down"}"#);
            let answer = match retry_after {
                Some(after) => answer.with_header("Retry-After", after),
                None => answer,
            };
            refused(DISCORD, answer)
        };

        assert_eq!(
            [Some("120"), None, Some("Wed, 21 Oct 2026 07:28:00 GMT")].map(limited),
            [
                "Discord is limiting this Server's requests; try again in 120 s",
                "Discord is limiting this Server's requests; try again in a few seconds",
                "Discord is limiting this Server's requests; try again in a few seconds",
            ]
        );
    }

    #[test]
    fn the_services_own_words_are_quoted() {
        let answer = Response::new(400, r#"{"message": "Invalid Webhook Token"}"#);

        assert_eq!(
            refused(DISCORD, answer),
            "Discord said: Invalid Webhook Token"
        );
    }

    #[test]
    fn a_refusal_the_reader_finds_no_words_in_is_a_failed_status() {
        let wordless = ["<html>Bad gateway</html>", r#"{"message": " "}"#]
            .map(|body| refused(DISCORD, Response::new(502, body)));

        assert_eq!(wordless, ["Discord failed with HTTP status 502"; 2]);
    }

    #[test]
    fn any_other_refusal_is_a_failed_status() {
        assert_eq!(
            refused(FANART, Response::new(500, "secret body")),
            "fanart.tv failed with HTTP status 500"
        );
    }

    #[test]
    fn a_404_is_found_to_be_nothing() {
        assert_eq!(FANART.found(Response::new(404, "")).unwrap(), None);
    }

    #[test]
    fn found_treats_other_statuses_as_accepted_does() {
        let ok = FANART.found(Response::new(200, "art")).unwrap();
        let unauthorized = FANART.found(Response::new(401, "")).unwrap_err();

        assert_eq!(ok, Some(Response::new(200, "art")));
        assert_eq!(unauthorized.to_string(), "fanart.tv refused the API key");
    }

    #[test]
    fn a_refusal_downcasts_with_its_status_and_seconds() {
        let answer = Response::new(429, "").with_header("Retry-After", "7");

        let failed = sent(FANART, answer).unwrap_err();

        let refusal = failed.downcast_ref::<Refusal>().unwrap();
        assert_eq!(
            (refusal.status(), refusal.retry_after_seconds()),
            (429, Some(7))
        );
    }
}
