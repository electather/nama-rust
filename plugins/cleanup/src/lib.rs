//! The Cleanup Installed Plugin: its hourly Job deletes from the Libraries the Admin chooses the
//! Requested Movies and Episodes each of whose Requesters is done with them, through the Kernel's
//! `delete_media` only (ADR 0048, ADR 0079). A title nobody Requested is kept. The Server builds
//! it for wasm32 and bundles it in its binary.

use std::collections::HashSet;

use nama_pdk::{
    Job, Library, Manifest, Permission, SettingField, SettingKind, TitleRef, TitleRequesters,
    TitleWatchStates, extism_pdk::Error, kernel, services, setting_field, setting_value,
};

const DAYS: &str = "days";
const DAYS_UNWATCHED: &str = "days_unwatched";
const LIBRARIES: &str = "libraries";

/// How many titles a Run reads, and asks the Requesters and Watch States of, at a time.
const PAGE_SIZE: u32 = 200;
/// The largest value the Kernel's Store keeps.
const MAX_STORED: usize = 64 * 1024;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// How long after asking to delete a title a Run asks again if the title is still listed and
/// due, since its delete, which runs in the background, must have failed.
const ASK_AGAIN_AFTER_MS: i64 = DAY_MS;

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Buffa, FnResult, JobRunner, Manifest, RunJob, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    /// Runs the one Job, `clean`, deleting what each of its Requesters is done with.
    pub(super) struct Cleanup;

    impl JobRunner for Cleanup {
        fn run(run: RunJob) -> Result<(), extism_pdk::Error> {
            super::clean_libraries(run.now_ms)
        }
    }

    nama_pdk::job_runner!(Cleanup);
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    Manifest {
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::JOB)],
        jobs: vec![Job {
            id: "clean".to_owned(),
            name: "Clean Libraries".to_owned(),
            description: "Deletes the Requested Movies and Episodes each of their Requesters is \
                          done with."
                .to_owned(),
            schedule: Some("0 * * * *".to_owned()),
            ..Job::default()
        }],
        permissions: vec![
            Permission::PERMISSION_STORE.into(),
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_REQUESTS.into(),
            Permission::PERMISSION_DELETE_MEDIA.into(),
        ],
        settings: vec![
            days_setting(
                setting_field(
                    DAYS,
                    "Days after the Requester watched it",
                    "A Requester is done with a Movie or Episode this many days after they \
                     watched it. It's deleted once all its Requesters are done, and a title \
                     nobody Requested is kept.",
                    SettingKind::SETTING_KIND_INTEGER,
                ),
                30,
            ),
            days_setting(
                setting_field(
                    DAYS_UNWATCHED,
                    "Days a Requester has to start watching",
                    "A Requester who hasn't watched a Movie or Episode is done with it this many \
                     days after it was added.",
                    SettingKind::SETTING_KIND_INTEGER,
                ),
                90,
            ),
            setting_field(
                LIBRARIES,
                "Libraries",
                "The Libraries to clean. None cleans none.",
                SettingKind::SETTING_KIND_LIBRARIES,
            ),
        ],
        name: "Cleanup".to_owned(),
        description: "Deletes the Requested Movies and Episodes each of their Requesters watched \
                      long ago, or didn't watch in time. Keeps what nobody Requested."
            .to_owned(),
        ..nama_pdk::manifest("cleanup", env!("CARGO_PKG_VERSION"))
    }
}

/// `field`, required, as a number of days of at least 0 defaulting to `default`.
fn days_setting(field: SettingField, default: i64) -> SettingField {
    SettingField {
        required: true,
        min_value: Some(0),
        default_value: nama_pdk::setting_default(setting_value::Value::IntegerValue(default)),
        ..field
    }
}

/// Cleans each Library the settings choose.
///
/// A refused delete doesn't stop the others: the Run fails afterwards with how many were refused
/// and the Kernel's words, so the Admin sees why in the log.
fn clean_libraries(now_ms: i64) -> Result<(), Error> {
    let ids = nama_pdk::setting_library_ids(LIBRARIES)?;
    // Before any Kernel call, so a Plugin enabled without Libraries chosen needs no grant yet.
    if ids.is_empty() {
        return Ok(());
    }
    let cutoffs = Cutoffs::at(
        now_ms,
        days(DAYS, &nama_pdk::required_setting(DAYS)?)?,
        days(DAYS_UNWATCHED, &nama_pdk::required_setting(DAYS_UNWATCHED)?)?,
    );
    let libraries = kernel::libraries()?;
    let mut refused = Vec::new();
    for library in chosen_libraries(&ids, &libraries) {
        refused.extend(clean(library, cutoffs, now_ms)?);
    }
    match refusal(&refused) {
        None => Ok(()),
        Some(refusal) => Err(Error::msg(refusal)),
    }
}

/// Deletes the due titles of `library` at `now_ms`, returning the Kernel's words for each delete it
/// refused.
fn clean(library: &Library, cutoffs: Cutoffs, now_ms: i64) -> Result<Vec<String>, Error> {
    let key = format!("deleting/{}", library.id);
    let recorded = decode(&kernel::store_get(&key, None)?.unwrap_or_default());
    let mut listed = HashSet::new();
    let mut due = Vec::new();
    let mut token = String::new();
    loop {
        let page = kernel::titles_page(library.id, PAGE_SIZE, &token)?;
        let (titles, added): (Vec<TitleRef>, Vec<i64>) = (page.titles.iter())
            .filter_map(|title| Some((title.title_ref.as_option()?.clone(), title.added_at_ms)))
            .unzip();
        listed.extend(titles.iter().map(Key::of));
        // The contract promises an answer for each title asked, not that it comes in the order
        // asked, so a pair naming another title is kept rather than judged.
        let judged =
            (kernel::requesters(&titles)?.into_iter()).zip(kernel::watch_states(&titles, None)?);
        for ((title, added_at_ms), (requesters, watched)) in
            titles.into_iter().zip(added).zip(judged)
        {
            let answers_name_it = requesters.title_ref.as_option() == Some(&title)
                && watched.title_ref.as_option() == Some(&title);
            if answers_name_it && cutoffs.is_due(added_at_ms, &requesters, &watched) {
                due.push(title);
            }
        }
        if page.next_page_token.is_empty() {
            break;
        }
        token = page.next_page_token;
    }
    if !due.is_empty() {
        nama_pdk::info!(
            "{} titles in Library {} are due for deleting",
            due.len(),
            library.id
        );
    }
    let asked = ask_due(recorded.clone(), &listed, &due, now_ms, |title, also| {
        kernel::delete_media(title.clone(), also)
    });
    let kept = encode(asked.recorded.into_iter());
    if decode(&kept) != recorded {
        kernel::store_put(&key, None, &kept)?;
    }
    Ok(asked.refused)
}

/// What [`ask_due`] did.
#[derive(Debug, Default, PartialEq, Eq)]
struct Asking {
    /// The titles the Store keeps as asked to delete, the latest last.
    recorded: Vec<Asked>,
    /// The Kernel's words for each delete it refused.
    refused: Vec<String>,
}

/// Deletes through [`delete_due`] each of `due` that `recorded` doesn't hold, after forgetting each
/// title of `recorded` no longer `listed`, since it is gone, and each asked [`ASK_AGAIN_AFTER_MS`]
/// or longer before `now_ms`, since its delete must have failed. The titles it asks are recorded
/// as asked at `now_ms`.
fn ask_due(
    recorded: Vec<Asked>,
    listed: &HashSet<Key>,
    due: &[TitleRef],
    now_ms: i64,
    delete: impl FnMut(&TitleRef, &[TitleRef]) -> Result<Vec<TitleRef>, Error>,
) -> Asking {
    let waiting: Vec<Asked> = (recorded.into_iter())
        .filter(|asked| {
            listed.contains(&asked.title) && now_ms.saturating_sub(asked.at_ms) < ASK_AGAIN_AFTER_MS
        })
        .collect();
    let considered = waiting.iter().map(|asked| asked.title.clone()).collect();
    let deleted = delete_due(due, &considered, delete);
    let now_asked = (deleted.recorded.into_iter()).map(|title| Asked {
        title,
        at_ms: now_ms,
    });
    Asking {
        recorded: waiting.into_iter().chain(now_asked).collect(),
        refused: deleted.refused,
    }
}

/// The Libraries among `libraries`, the ones the Kernel lists, whose ids `ids` holds, each once and
/// in `libraries`' order. An id no Library has, such as a deleted one's, picks nothing.
fn chosen_libraries<'a>(ids: &[i64], libraries: &'a [Library]) -> Vec<&'a Library> {
    (libraries.iter())
        .filter(|library| ids.contains(&library.id))
        .collect()
}

/// The setting `key`, which holds `setting`, as a number of days, at least 0. A failure names the
/// setting. The Kernel refuses saving one under 0, but one saved before the Manifest said so may
/// still be stored.
fn days(key: &str, setting: &str) -> Result<i64, Error> {
    match setting.trim().parse::<i64>() {
        Ok(days) if days >= 0 => Ok(days),
        _ => Err(Error::msg(format!(
            "the setting {key} must be a whole number of 0 or more, not {setting}"
        ))),
    }
}

const fn days_before(now_ms: i64, days: i64) -> i64 {
    now_ms.saturating_sub(days.saturating_mul(DAY_MS))
}

/// The latest times at which a Requester is done with a title.
#[derive(Debug, Clone, Copy)]
struct Cutoffs {
    /// For one who played it: when their Watch State last changed.
    watched_ms: i64,
    /// For one who didn't: when the title was added.
    unwatched_ms: i64,
}

impl Cutoffs {
    /// The cutoffs `days` and `days_unwatched` days before `now_ms`.
    const fn at(now_ms: i64, days: i64, days_unwatched: i64) -> Self {
        Self {
            watched_ms: days_before(now_ms, days),
            unwatched_ms: days_before(now_ms, days_unwatched),
        }
    }

    /// Whether a title added at `added_at_ms` is due: it was Requested, and each of its
    /// `requesters` is done with it, judged by their own Watch State in `watched` alone. One whose
    /// Requesters were all deleted is due once the unwatched cutoff passed since it was added.
    fn is_due(
        self,
        added_at_ms: i64,
        requesters: &TitleRequesters,
        watched: &TitleWatchStates,
    ) -> bool {
        let unwatched_long_enough = added_at_ms <= self.unwatched_ms;
        let is_done = |&user: &i64| {
            let played =
                (watched.states.iter()).find(|state| state.user_id == user && state.played);
            played.map_or(unwatched_long_enough, |played| {
                played.updated_at_ms <= self.watched_ms
            })
        };
        requesters.requested
            && if requesters.user_ids.is_empty() {
                unwatched_long_enough
            } else {
                requesters.user_ids.iter().all(is_done)
            }
    }
}

/// What [`delete_due`] did.
#[derive(Debug, Default, PartialEq, Eq)]
struct Deleted {
    /// The titles whose deletes it started, and those they took along.
    recorded: Vec<Key>,
    /// The Kernel's words for each delete it refused.
    refused: Vec<String>,
}

/// Deletes each of `due` through `delete`, skipping those `considered` holds and those an earlier
/// delete took along, so nothing is deleted twice. Each delete names the other due titles as the
/// ones it may take along, so Episodes sharing a file go together.
fn delete_due(
    due: &[TitleRef],
    considered: &HashSet<Key>,
    mut delete: impl FnMut(&TitleRef, &[TitleRef]) -> Result<Vec<TitleRef>, Error>,
) -> Deleted {
    let mut done = considered.clone();
    let mut deleted = Deleted::default();
    for (index, title) in due.iter().enumerate() {
        let key = Key::of(title);
        if done.contains(&key) {
            continue;
        }
        let also: Vec<TitleRef> = (due.iter().enumerate())
            .filter(|&(other, _)| other != index)
            .map(|(_, other)| other.clone())
            .collect();
        match delete(title, &also) {
            Ok(taken_along) => {
                let started = std::iter::once(key).chain(taken_along.iter().map(Key::of));
                (deleted.recorded).extend(started.filter(|started| done.insert(started.clone())));
            }
            Err(error) => deleted.refused.push(error.to_string()),
        }
    }
    deleted
}

/// The Run's error for `refused`, the Kernel's words for each refused delete, each once; `None`
/// when nothing was refused.
fn refusal(refused: &[String]) -> Option<String> {
    if refused.is_empty() {
        return None;
    }
    let mut words: Vec<&str> = Vec::new();
    for said in refused {
        if !words.contains(&said.as_str()) {
            words.push(said);
        }
    }
    Some(format!(
        "the Kernel refused {} of the deletes: {}",
        refused.len(),
        words.join("; ")
    ))
}

/// A title within one Library, as the Store keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    kind: i32,
    namespace: String,
    value: String,
}

impl Key {
    fn of(title: &TitleRef) -> Self {
        let id = title.id.as_option();
        Self {
            kind: title.kind.to_i32(),
            namespace: id.map_or_default(|id| id.namespace.clone()),
            value: id.map_or_default(|id| id.value.clone()),
        }
    }

    /// The `TitleRef` it names in `library_id`.
    #[cfg(test)]
    fn title_ref(&self, library_id: i64) -> TitleRef {
        TitleRef {
            library_id,
            kind: self.kind.into(),
            id: nama_pdk::TitleId {
                namespace: self.namespace.clone(),
                value: self.value.clone(),
                ..nama_pdk::TitleId::default()
            }
            .into(),
            ..TitleRef::default()
        }
    }
}

/// A title a Run asked the Kernel to delete, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    title: Key,
    at_ms: i64,
}

impl Asked {
    /// The line the Store keeps it as: its title's kind, namespace and value, then when it was
    /// asked, separated by tabs.
    fn line(&self) -> String {
        let Key {
            kind,
            namespace,
            value,
        } = &self.title;
        format!("{kind}\t{namespace}\t{value}\t{}\n", self.at_ms)
    }
}

/// `asked` as a Store value, one line each, leaving out the first ones when they don't all fit,
/// since the latest are the ones whose deletes may still be going on.
fn encode(asked: impl Iterator<Item = Asked>) -> Vec<u8> {
    let mut lines: Vec<String> = asked.map(|asked| asked.line()).collect();
    let mut size = 0;
    let fitting = lines
        .iter()
        .rev()
        .take_while(|line| {
            size += line.len();
            size <= MAX_STORED
        })
        .count();
    lines
        .split_off(lines.len().saturating_sub(fitting))
        .concat()
        .into_bytes()
}

/// The titles a Store value [`encode`] made holds, leaving out a line it can't read, such as one
/// of the earlier form without the time it was asked.
fn decode(value: &[u8]) -> Vec<Asked> {
    (String::from_utf8_lossy(value).lines())
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let title = Key {
                kind: fields.next()?.parse().ok()?,
                namespace: fields.next()?.to_owned(),
                value: fields.next()?.to_owned(),
            };
            let at_ms = fields.next()?.parse().ok()?;
            fields.next().is_none().then_some(Asked { title, at_ms })
        })
        .collect()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
