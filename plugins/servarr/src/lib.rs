//! What the Sonarr and Radarr Installed Plugins share. "Servarr" is upstream's name for the
//! codebase and v3 API both services are built on, not a Nama term.

use nama_pdk::extism_pdk::{Error, HttpRequest, HttpResponse, Json, json};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// A Sonarr or Radarr service's API, as its URL, API key and name say. Its errors name the
/// service, and never its URL: a Request's reason reaches every Requester.
pub struct Servarr {
    name: &'static str,
    url: String,
    api_key: String,
}

// Not derived, so the API key stays out of logs.
impl std::fmt::Debug for Servarr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Servarr")
            .field("name", &self.name)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl Servarr {
    /// The service `name`, such as "Sonarr", at `url` with the key `api_key`.
    #[must_use]
    pub fn new(name: &'static str, url: String, api_key: String) -> Self {
        Self { name, url, api_key }
    }

    /// What the service answers at the API path `path`, such as `qualityprofile`.
    ///
    /// # Errors
    /// If the service doesn't answer, refuses, or answers with something other than a `T`.
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        self.parsed(&self.get_response(path)?)
    }

    /// The service's response at the API path `path`, if it took the request, for its headers.
    ///
    /// # Errors
    /// If the service doesn't answer or refuses.
    pub fn get_response(&self, path: &str) -> Result<HttpResponse, Error> {
        self.accepted(self.send("GET", path, None::<()>)?)
    }

    /// `response`'s body as a `T`.
    ///
    /// # Errors
    /// If the body isn't a `T`.
    pub fn parsed<T: DeserializeOwned>(&self, response: &HttpResponse) -> Result<T, Error> {
        parsed(self.name, &response.body())
    }

    /// Sends the service the change `body` to the API path `path` with `method`, ignoring what it
    /// answers once it takes it.
    ///
    /// # Errors
    /// If the service doesn't answer or refuses.
    pub fn change(&self, method: &str, path: &str, body: impl Serialize) -> Result<(), Error> {
        self.accepted(self.send(method, path, Some(body))?)
            .map(drop)
    }

    /// Posts `body` to the API path `path` to add what it describes. Returns `false` if the service
    /// already has it, as it says by the validator `exists`, such as `MovieExistsValidator`.
    ///
    /// # Errors
    /// If the service doesn't answer or refuses for another reason.
    pub fn add(&self, path: &str, body: impl Serialize, exists: &str) -> Result<bool, Error> {
        let response = self.send("POST", path, Some(body))?;
        if already_has(exists, response.status_code(), &response.body()) {
            return Ok(false);
        }
        self.accepted(response).map(|_| true)
    }

    fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<impl Serialize>,
    ) -> Result<HttpResponse, Error> {
        let url = format!("{}/api/v3/{path}", self.url.trim_end_matches('/'));
        let request = HttpRequest::new(url)
            .with_method(method)
            .with_header("X-Api-Key", &self.api_key)
            .with_header("Content-Type", "application/json");
        let unreachable = format!("{} didn't answer", self.name);
        nama_pdk::http_request(&request, body.map(Json), &unreachable)
    }

    /// `response`, if the service took the request, or else why it refused.
    fn accepted(&self, response: HttpResponse) -> Result<HttpResponse, Error> {
        let status = response.status_code();
        if (200..300).contains(&status) {
            Ok(response)
        } else {
            Err(refused(self.name, status, &response.body()))
        }
    }
}

/// `body` as a `T`, or else that the service `service` made no sense.
fn parsed<T: DeserializeOwned>(service: &str, body: &[u8]) -> Result<T, Error> {
    json::from_slice(body)
        .map_err(|err| Error::msg(format!("{service}'s answer made no sense: {err}")))
}

/// Whether a response with `status` and `body` says the service already has what it was asked
/// to add, by the validator `exists`.
fn already_has(exists: &str, status: u16, body: &[u8]) -> bool {
    failures(status, body)
        .iter()
        .any(|failure| failure.error_code.as_deref() == Some(exists))
}

/// Why the service `service` refused a request, from the `status` and `body` of its response, in
/// words a Requester understands.
fn refused(service: &str, status: u16, body: &[u8]) -> Error {
    let messages: Vec<_> = failures(status, body)
        .into_iter()
        .map(|failure| failure.error_message)
        .collect();
    Error::msg(match status {
        401 => format!("{service} refused the API key"),
        400 if !messages.is_empty() => format!("{service} said: {}", messages.join(" ")),
        status => format!("{service} failed with HTTP status {status}"),
    })
}

/// The validation failures in `body`, if `status` is a `400 Bad Request`.
fn failures(status: u16, body: &[u8]) -> Vec<ValidationFailure> {
    if status == 400 {
        json::from_slice(body).unwrap_or_default()
    } else {
        Vec::new()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidationFailure {
    error_message: String,
    error_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAILURES: &[u8] = br#"[
        {"errorMessage": "This movie has already been added", "errorCode": "MovieExistsValidator"},
        {"errorMessage": "Path is not valid", "errorCode": null}
    ]"#;

    #[test]
    fn a_refused_key_says_so() {
        assert_eq!(
            refused("Sonarr", 401, b"").to_string(),
            "Sonarr refused the API key"
        );
    }

    #[test]
    fn a_bad_request_says_its_validation_messages() {
        assert_eq!(
            refused("Radarr", 400, FAILURES).to_string(),
            "Radarr said: This movie has already been added Path is not valid"
        );
    }

    #[test]
    fn a_bad_request_without_messages_says_its_status() {
        assert_eq!(
            refused("Radarr", 400, b"not json").to_string(),
            "Radarr failed with HTTP status 400"
        );
    }

    #[test]
    fn another_status_says_it() {
        assert_eq!(
            refused("Sonarr", 503, FAILURES).to_string(),
            "Sonarr failed with HTTP status 503"
        );
    }

    #[test]
    fn already_has_it_when_a_bad_request_names_the_validator() {
        assert!(already_has("MovieExistsValidator", 400, FAILURES));
    }

    #[test]
    fn already_has_it_only_by_that_validator() {
        assert!(!already_has("SeriesExistsValidator", 400, FAILURES));
    }

    #[test]
    fn already_has_it_only_in_a_bad_request() {
        assert!(!already_has("MovieExistsValidator", 409, FAILURES));
    }

    #[test]
    fn an_answer_of_the_wrong_shape_made_no_sense() {
        let error = parsed::<Vec<i64>>("Sonarr", b"{}").unwrap_err().to_string();
        assert!(
            error.starts_with("Sonarr's answer made no sense: "),
            "{error}"
        );
    }

    #[test]
    fn an_answer_of_the_right_shape_is_parsed() {
        assert_eq!(
            parsed::<Vec<i64>>("Sonarr", b"[1, 2]").ok(),
            Some(vec![1, 2])
        );
    }
}
