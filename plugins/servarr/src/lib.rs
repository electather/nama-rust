//! What the Sonarr and Radarr Installed Plugins share. "Servarr" is upstream's name for the
//! codebase and v3 API both services are built on, not a Nama term.

use std::collections::{HashMap, HashSet};

use nama_pdk::{
    CheckedSettings, Download, Fulfilled, Request,
    extism_pdk::{Error, HttpRequest, HttpResponse, Json, config, json},
    nama_proto::nama::{
        plugin::v1::{SettingField, SettingKind},
        v1::DownloadStatus,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const URL: &str = "url";
const API_KEY: &str = "api_key";
const QUALITY_PROFILE: &str = "quality_profile";
const ROOT_FOLDER: &str = "root_folder";

/// The four settings of the service `service`, which adds each `added`, such as "Movie", and
/// listens on `port` by default.
#[must_use]
pub fn settings(service: &str, added: &str, port: u16) -> Vec<SettingField> {
    let setting = |key: &str, label: &str, description: String, kind: SettingKind| SettingField {
        key: key.to_owned(),
        label: label.to_owned(),
        description,
        kind: kind.into(),
        required: true,
        ..SettingField::default()
    };
    vec![
        setting(
            URL,
            "URL",
            format!("Where {service} is, such as http://localhost:{port}."),
            SettingKind::SETTING_KIND_URL,
        ),
        setting(
            API_KEY,
            "API key",
            format!("{service}'s API key, from Settings > General in {service}."),
            SettingKind::SETTING_KIND_SECRET,
        ),
        setting(
            QUALITY_PROFILE,
            "Quality profile",
            format!("The quality {service} downloads each {added} in."),
            SettingKind::SETTING_KIND_CHOICE,
        ),
        setting(
            ROOT_FOLDER,
            "Root folder",
            format!("The folder {service} puts each {added} in."),
            SettingKind::SETTING_KIND_CHOICE,
        ),
    ]
}

/// The root folder the settings say the service adds into.
///
/// # Errors
/// If it isn't set.
pub fn root_folder() -> Result<String, Error> {
    setting(ROOT_FOLDER)
}

/// The quality profile id `request` asks for: its Quality's, else the setting's.
///
/// # Errors
/// If neither is set, or it isn't an id.
pub fn quality_profile(request: &Request) -> Result<i64, Error> {
    let profile = match request.quality.as_option() {
        Some(quality) => quality.profile_id.clone(),
        None => setting(QUALITY_PROFILE)?,
    };
    profile
        .parse()
        .map_err(|err| Error::msg(format!("the quality profile isn't an id: {err}")))
}

/// What a hand-off reports: its `reference`, and the profile `profile_id` the service has what it
/// was handed at, named as in `profiles`.
#[must_use]
pub fn fulfilled(
    reference: String,
    profile_id: Option<i64>,
    profiles: Vec<QualityProfile>,
) -> Fulfilled {
    let used = profiles
        .into_iter()
        .find(|profile| Some(profile.id) == profile_id);
    Fulfilled {
        reference,
        profile_id: profile_id.map(|id| id.to_string()).unwrap_or_default(),
        profile_name: used.map(|used| used.name).unwrap_or_default(),
        ..Fulfilled::default()
    }
}

/// The setting `key`, which the Server sets before every call but a settings check, and for
/// that one too unless it is a Choice.
fn setting(key: &str) -> Result<String, Error> {
    config::get(key)?.ok_or_else(|| Error::msg(format!("the setting {key} isn't set")))
}

/// A quality profile of the service, which a Request is added at.
#[derive(Debug, Deserialize)]
pub struct QualityProfile {
    /// The service's id for it.
    pub id: i64,
    /// What an admin knows it by.
    pub name: String,
}

#[derive(Deserialize)]
struct RootFolder {
    path: String,
}

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
    /// The service `name`, such as "Sonarr", where the settings say and with their API key.
    ///
    /// # Errors
    /// If either isn't set.
    pub fn from_settings(name: &'static str) -> Result<Self, Error> {
        Ok(Self {
            name,
            url: setting(URL)?,
            api_key: setting(API_KEY)?,
        })
    }

    /// The service's quality profiles.
    ///
    /// # Errors
    /// If the service doesn't answer, refuses, or answers with something other than profiles.
    pub fn quality_profiles(&self) -> Result<Vec<QualityProfile>, Error> {
        self.get("qualityprofile")
    }

    /// The settings check: the service's quality profiles, by id, and its root folders, by path,
    /// to choose from.
    ///
    /// # Errors
    /// If the service doesn't answer, refuses, or answers with something other than them.
    pub fn checked_settings(&self) -> Result<CheckedSettings, Error> {
        let profiles: Vec<_> = self
            .quality_profiles()?
            .into_iter()
            .map(|profile| (profile.id.to_string(), profile.name))
            .collect();
        let folders: Vec<RootFolder> = self.get("rootfolder")?;
        let folders = folders
            .into_iter()
            .map(|folder| (folder.path.clone(), folder.path))
            .collect();
        Ok(nama_pdk::checked_settings([
            (QUALITY_PROFILE, profiles),
            (ROOT_FOLDER, folders),
        ]))
    }

    /// Why `request` can't go to the service, which has no quality profile `id`.
    #[must_use]
    pub fn no_such_profile(&self, request: &Request, id: i64) -> Error {
        let reason = format!("{} has no quality profile with the id {id}", self.name);
        Error::msg(match request.quality.as_option() {
            Some(quality) => format!("{reason}, which {} uses", quality.name),
            None => reason,
        })
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

    /// The Downloads in the service's queue for each of `references` that has any, read in one
    /// call.
    ///
    /// # Errors
    /// If the service doesn't answer, refuses, or answers with something other than a queue.
    pub fn progress<Q: Queued>(
        &self,
        references: &[String],
    ) -> Result<HashMap<String, Vec<Download>>, Error> {
        // ponytail: reads only the first 1000 entries, which Sonarr lists one per Episode; page
        // through `totalRecords` if a queue ever holds more.
        let queue: Queue<Q> = self.get(&format!("queue?pageSize=1000&{}=true", Q::INCLUDE))?;
        Ok(downloads(self.name, queue.records, references))
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

/// What a queue entry is for, as each Plugin reads its own part of the entry.
pub trait Queued: DeserializeOwned {
    /// The query parameter that has the service add this part to each entry, such as
    /// `includeMovie`.
    const INCLUDE: &'static str;

    /// The reference `fulfill` gave what the entry is for, if the entry says.
    fn reference(&self) -> Option<String>;
}

/// The Downloads in the queue `entries` of the service `service` for each of `references`, one
/// per reference, download id and title.
fn downloads<Q: Queued>(
    service: &str,
    entries: Vec<QueueEntry<Q>>,
    references: &[String],
) -> HashMap<String, Vec<Download>> {
    let mut listed = HashSet::new();
    let mut downloads: HashMap<String, Vec<Download>> = HashMap::new();
    for entry in entries {
        let Some(reference) = entry.part.reference().filter(|r| references.contains(r)) else {
            continue;
        };
        // Sonarr lists a Season pack once for each of its Episodes.
        let download = (entry.download_id.clone(), entry.title.clone());
        if listed.insert((reference.clone(), download)) {
            downloads
                .entry(reference)
                .or_default()
                .push(entry.into_download(service));
        }
    }
    downloads
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

/// A page of `GET /api/v3/queue`.
#[derive(Deserialize)]
#[serde(bound = "Q: Queued")]
struct Queue<Q> {
    records: Vec<QueueEntry<Q>>,
}

/// One download in the service's queue, or for Sonarr one Episode of it, so a Season pack has an
/// entry for each. The service may send `null` for those that are `Option`s.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", bound = "Q: Queued")]
struct QueueEntry<Q> {
    /// What it is for, only there with the part's include parameter.
    #[serde(flatten)]
    part: Q,
    /// The download client's id for the download, shared by a pack's entries.
    download_id: Option<String>,
    title: Option<String>,
    size: f64,
    sizeleft: f64,
    /// How long it has left, as a .NET `TimeSpan`.
    timeleft: Option<String>,
    /// Where the download client has it, such as `downloading` or `completed`.
    status: String,
    /// `ok`, `warning` or `error`.
    tracked_download_status: Option<String>,
    /// Where the service has it, such as `downloading`, `importBlocked` or `importing`.
    tracked_download_state: Option<String>,
    status_messages: Option<Vec<StatusMessage>>,
    error_message: Option<String>,
}

#[derive(Deserialize)]
struct StatusMessage {
    messages: Option<Vec<String>>,
}

impl<Q> QueueEntry<Q> {
    /// The Download it is, with what is wrong in the words of the service `service`.
    fn into_download(self, service: &str) -> Download {
        let status = self.status();
        Download {
            percent: self.percent(),
            size_bytes: self.size_bytes(),
            // The service gives a queued download's time left as zero.
            eta_seconds: (status == DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING)
                .then(|| self.timeleft.as_deref().and_then(seconds_of))
                .flatten(),
            attention_reason: (status == DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION)
                .then(|| self.attention_reason(service))
                .flatten(),
            status: status.into(),
            title: self.title.unwrap_or_default(),
            ..Download::default()
        }
    }

    fn status(&self) -> DownloadStatus {
        let tracked_state = self.tracked_download_state.as_deref();
        if self.needs_attention() {
            DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION
        } else if self.status == "completed"
            || matches!(
                tracked_state,
                Some("importPending" | "importing" | "imported")
            )
        {
            DownloadStatus::DOWNLOAD_STATUS_IMPORTING
        } else if self.status == "downloading" {
            DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING
        } else {
            DownloadStatus::DOWNLOAD_STATUS_QUEUED
        }
    }

    /// Whether it waits on a person, rather than on the service or its download client.
    fn needs_attention(&self) -> bool {
        matches!(
            self.tracked_download_status.as_deref(),
            Some("warning" | "error")
        ) || matches!(
            self.status.as_str(),
            "failed" | "warning" | "downloadClientUnavailable"
        ) || matches!(
            self.tracked_download_state.as_deref(),
            Some("importBlocked" | "failedPending" | "failed")
        )
    }

    /// The messages of the service `service` about it, if it has any.
    fn attention_reason(&self, service: &str) -> Option<String> {
        let mut messages: Vec<&str> = self
            .status_messages
            .iter()
            .flatten()
            .flat_map(|status| status.messages.iter().flatten())
            .map(String::as_str)
            .collect();
        messages.extend(self.error_message.as_deref());
        (!messages.is_empty()).then(|| format!("{service} said: {}", messages.join(" ")))
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "a percent needs no more than f32's precision"
    )]
    fn percent(&self) -> f32 {
        if self.size > 0.0 {
            ((self.size - self.sizeleft) / self.size * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the service counts bytes in a double; only a positive size is kept"
    )]
    fn size_bytes(&self) -> Option<u64> {
        (self.size > 0.0).then_some(self.size as u64)
    }
}

/// The seconds in a .NET `TimeSpan` such as `1.02:03:04`, dropping any fraction of a second.
fn seconds_of(timespan: &str) -> Option<u64> {
    let mut parts = timespan.split(':');
    let (hours, minutes, seconds) = (parts.next()?, parts.next()?, parts.next()?);
    let (days, hours) = hours.split_once('.').unwrap_or(("0", hours));
    let seconds = seconds.split('.').next()?;
    let [days, hours, minutes, seconds] =
        [days, hours, minutes, seconds].map(|part| part.parse::<u64>().ok());
    Some(days? * 86_400 + hours? * 3_600 + minutes? * 60 + seconds?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidationFailure {
    error_message: String,
    error_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use nama_pdk::nama_proto::nama::v1::RequestedQuality;

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

    /// Sonarr, which no test reaches over the network.
    fn sonarr() -> Servarr {
        Servarr {
            name: "Sonarr",
            url: String::new(),
            api_key: String::new(),
        }
    }

    /// A Request at the Quality `name`.
    fn requested_at(name: &str) -> Request {
        Request {
            quality: RequestedQuality {
                name: name.to_owned(),
                ..RequestedQuality::default()
            }
            .into(),
            ..Request::default()
        }
    }

    fn profiles() -> Vec<QualityProfile> {
        vec![
            QualityProfile {
                id: 1,
                name: "Any".to_owned(),
            },
            QualityProfile {
                id: 4,
                name: "HD-1080p".to_owned(),
            },
        ]
    }

    #[test]
    fn settings_name_the_service_its_port_and_what_it_adds() {
        let described: Vec<_> = settings("Radarr", "Movie", 7878)
            .into_iter()
            .map(|field| (field.key, field.description))
            .collect();
        assert_eq!(
            described,
            [
                ("url", "Where Radarr is, such as http://localhost:7878."),
                (
                    "api_key",
                    "Radarr's API key, from Settings > General in Radarr."
                ),
                (
                    "quality_profile",
                    "The quality Radarr downloads each Movie in."
                ),
                ("root_folder", "The folder Radarr puts each Movie in."),
            ]
            .map(|(key, description)| (key.to_owned(), description.to_owned()))
        );
    }

    #[test]
    fn no_such_profile_without_a_quality_names_only_the_id() {
        assert_eq!(
            sonarr().no_such_profile(&Request::default(), 4).to_string(),
            "Sonarr has no quality profile with the id 4"
        );
    }

    #[test]
    fn no_such_profile_names_the_quality_that_uses_it() {
        assert_eq!(
            sonarr().no_such_profile(&requested_at("HD"), 4).to_string(),
            "Sonarr has no quality profile with the id 4, which HD uses"
        );
    }

    #[test]
    fn a_hand_off_names_the_profile_it_went_out_at() {
        let reported = fulfilled("603".to_owned(), Some(4), profiles());
        assert_eq!(
            (
                reported.reference.as_str(),
                reported.profile_id.as_str(),
                reported.profile_name.as_str()
            ),
            ("603", "4", "HD-1080p")
        );
    }

    #[test]
    fn a_hand_off_without_a_profile_names_none() {
        let reported = fulfilled("603".to_owned(), None, profiles());
        assert_eq!(
            (reported.profile_id.as_str(), reported.profile_name.as_str()),
            ("", "")
        );
    }

    #[test]
    fn an_answer_of_the_wrong_shape_made_no_sense() {
        let error = parsed::<Vec<i64>>("Sonarr", b"{}").unwrap_err().to_string();
        assert!(
            error.starts_with("Sonarr's answer made no sense: "),
            "{error}"
        );
    }

    /// A queue entry's part that gives its reference as it is.
    #[derive(Deserialize)]
    struct Referenced {
        reference: Option<String>,
    }

    impl Queued for Referenced {
        const INCLUDE: &'static str = "includeNothing";

        fn reference(&self) -> Option<String> {
            self.reference.clone()
        }
    }

    /// The Downloads Sonarr's queue `entries`, a JSON array, holds for `references`.
    fn downloads_of(entries: &str, references: &[&str]) -> HashMap<String, Vec<Download>> {
        let entries: Vec<QueueEntry<Referenced>> =
            parsed("Sonarr", entries.as_bytes()).expect("the entries are a queue's");
        let references: Vec<String> = references.iter().map(|&r| r.to_owned()).collect();
        downloads("Sonarr", entries, &references)
    }

    /// The one Download `entry` is, for the reference `1`.
    fn download(entry: &str) -> Download {
        let mut downloads = downloads_of(&format!("[{entry}]"), &["1"]);
        let mut listed = downloads.remove("1").expect("the entry is listed");
        assert_eq!(listed.len(), 1);
        listed.remove(0)
    }

    #[test]
    fn a_downloading_entry_says_how_far_along_it_is() {
        let entry = r#"{"reference": "1", "title": "Show.S01", "size": 1000, "sizeleft": 250,
            "timeleft": "00:01:30", "status": "downloading", "trackedDownloadStatus": "ok",
            "trackedDownloadState": "downloading"}"#;
        assert_eq!(
            download(entry),
            Download {
                title: "Show.S01".to_owned(),
                percent: 75.0,
                size_bytes: Some(1000),
                eta_seconds: Some(90),
                status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
                ..Download::default()
            }
        );
    }

    #[test]
    fn a_queued_entry_of_unknown_size_has_no_size_or_time_left() {
        let entry = r#"{"reference": "1", "title": null, "size": 0, "sizeleft": 0,
            "timeleft": "00:00:00", "status": "queued"}"#;
        assert_eq!(
            download(entry),
            Download {
                status: DownloadStatus::DOWNLOAD_STATUS_QUEUED.into(),
                ..Download::default()
            }
        );
    }

    #[test]
    fn a_completed_entry_is_importing() {
        let entry = r#"{"reference": "1", "size": 10, "sizeleft": 0, "status": "completed",
            "trackedDownloadState": "importPending"}"#;
        assert_eq!(
            download(entry).status,
            DownloadStatus::DOWNLOAD_STATUS_IMPORTING
        );
    }

    #[test]
    fn an_entry_waiting_on_a_person_says_why_in_the_services_words() {
        let entry = r#"{"reference": "1", "size": 10, "sizeleft": 0, "timeleft": "00:00:10",
            "status": "completed", "trackedDownloadStatus": "warning",
            "trackedDownloadState": "importBlocked",
            "statusMessages": [{"messages": ["No files found"]}, {"messages": null}],
            "errorMessage": "Check the download"}"#;
        let download = download(entry);
        assert_eq!(
            download.status,
            DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION
        );
        assert_eq!(
            download.attention_reason.as_deref(),
            Some("Sonarr said: No files found Check the download")
        );
        assert_eq!(download.eta_seconds, None);
    }

    #[test]
    fn an_entry_waiting_on_a_person_without_messages_gives_no_reason() {
        let entry = r#"{"reference": "1", "size": 10, "sizeleft": 5, "status": "failed"}"#;
        assert_eq!(download(entry).attention_reason, None);
    }

    #[test]
    fn a_season_packs_entries_merge_into_one_download_each() {
        let entries = r#"[
            {"reference": "1", "downloadId": "a", "title": "Show.S01", "size": 10, "sizeleft": 5, "status": "downloading"},
            {"reference": "1", "downloadId": "a", "title": "Show.S01", "size": 10, "sizeleft": 5, "status": "downloading"},
            {"reference": "1", "downloadId": "b", "title": "Show.S01", "size": 10, "sizeleft": 5, "status": "downloading"},
            {"reference": "1", "downloadId": "b", "title": "Show.S01.REPACK", "size": 10, "sizeleft": 5, "status": "downloading"},
            {"reference": "2", "downloadId": "a", "title": "Show.S01", "size": 10, "sizeleft": 5, "status": "downloading"}
        ]"#;
        let downloads = downloads_of(entries, &["1", "2"]);
        assert_eq!(downloads["1"].len(), 3);
        assert_eq!(downloads["2"].len(), 1);
    }

    #[test]
    fn entries_for_other_references_are_left_out() {
        let entries = r#"[
            {"reference": "2", "size": 10, "sizeleft": 5, "status": "downloading"},
            {"reference": null, "size": 10, "sizeleft": 5, "status": "downloading"}
        ]"#;
        assert!(downloads_of(entries, &["1"]).is_empty());
    }

    #[test]
    fn a_timespan_counts_its_days_hours_minutes_and_seconds() {
        assert_eq!(seconds_of("1.02:03:04"), Some(93_784));
    }

    #[test]
    fn a_timespan_under_a_day_drops_its_fraction_of_a_second() {
        assert_eq!(seconds_of("00:01:30.9990000"), Some(90));
    }

    #[test]
    fn a_timespan_of_another_shape_has_no_seconds() {
        assert_eq!(seconds_of("soon"), None);
        assert_eq!(seconds_of("01:xx:00"), None);
    }

    #[test]
    fn an_answer_of_the_right_shape_is_parsed() {
        assert_eq!(
            parsed::<Vec<i64>>("Sonarr", b"[1, 2]").ok(),
            Some(vec![1, 2])
        );
    }
}
