//! The Cleanup Installed Plugin: on each tick, deletes the Movies and Episodes every User played at
//! least `days` days ago from the Libraries the Admin names, through the Kernel's `delete_media`
//! only (ADR 0048). The Server builds it for wasm32 and bundles it in its binary.

use std::collections::HashSet;

use nama_pdk::{
    Library, Manifest, Permission, Tick, TitleRef, TitleWatchStates, WatchState,
    extism_pdk::{Error, config},
    kernel,
    nama_plugin_proto::nama::plugin::v1::{SettingField, SettingKind, SettingValue, setting_value},
    services,
};

const DAYS: &str = "days";
const LIBRARIES: &str = "libraries";

/// How many titles the tick reads, and asks the Watch States of, at a time.
const PAGE_SIZE: u32 = 200;
/// The largest value the Kernel's Store keeps.
const MAX_STORED: usize = 64 * 1024;
/// The longest key the Kernel's Store keeps, in bytes.
const MAX_KEY: usize = 256;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// How long after asking to delete a title the tick asks again if the title is still listed and
/// due, since its delete, which runs in the background, must have failed.
const ASK_AGAIN_AFTER_MS: i64 = DAY_MS;

/// The Plugin's exports. `plugin_fn` drops their doc comments and attributes, so lints about the
/// code it generates are expected here.
#[expect(
    clippy::unnecessary_wraps,
    reason = "an export returns `FnResult` even when it can't fail"
)]
mod exports {
    use nama_pdk::{Automation, Buffa, FnResult, Manifest, Tick, extism_pdk, plugin_fn};

    #[plugin_fn]
    pub fn manifest() -> FnResult<Buffa<Manifest>> {
        Ok(Buffa(super::manifest()))
    }

    /// Deletes, on each tick, what every User watched long ago.
    struct Cleanup;

    impl Automation for Cleanup {
        fn tick(tick: Tick) -> Result<(), extism_pdk::Error> {
            super::tick(&tick)
        }
    }

    nama_pdk::automation!(Cleanup);
}

/// What `manifest()` returns, kept apart from the export so a unit test can read it.
fn manifest() -> Manifest {
    let setting = |key: &str, label: &str, description: &str, kind: SettingKind| SettingField {
        key: key.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: kind.into(),
        ..SettingField::default()
    };
    Manifest {
        logo_svg: include_str!("../logo.svg").to_owned(),
        points: vec![nama_pdk::point(services::AUTOMATION)],
        permissions: vec![
            Permission::PERMISSION_STORE.into(),
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_USERS.into(),
            Permission::PERMISSION_DELETE_MEDIA.into(),
        ],
        tick_every_seconds: 3600,
        settings: vec![
            SettingField {
                required: true,
                default_value: SettingValue {
                    value: Some(setting_value::Value::IntegerValue(30)),
                    ..SettingValue::default()
                }
                .into(),
                ..setting(
                    DAYS,
                    "Days",
                    "Delete a Movie or Episode once every User watched it at least this many \
                     days ago.",
                    SettingKind::SETTING_KIND_INTEGER,
                )
            },
            setting(
                LIBRARIES,
                "Libraries",
                "The names of the Libraries to clean, separated by commas. Empty cleans none. \
                 Every Library with a listed name is cleaned. One renamed after a tick found it \
                 is still cleaned under its old name, until the only Libraries with that name \
                 are ones no tick found under it.",
                SettingKind::SETTING_KIND_TEXT,
            ),
        ],
        name: "Cleanup".to_owned(),
        description: "Deletes the Movies and Episodes every User watched long ago.".to_owned(),
        ..nama_pdk::manifest("cleanup", env!("CARGO_PKG_VERSION"))
    }
}

/// Cleans each Library the settings name.
///
/// A refused delete doesn't stop the others: the tick fails afterwards with how many were refused
/// and the Kernel's words, so the Admin sees why in the log.
fn tick(tick: &Tick) -> Result<(), Error> {
    let named = config::get(LIBRARIES)?.unwrap_or_default();
    let named = library_names(&named);
    // Before any Kernel call, so a Plugin enabled without Libraries named needs no grant yet.
    if named.is_empty() {
        return Ok(());
    }
    let cutoff_ms = cutoff_ms(tick.now_ms, days(config::get(DAYS)?.as_deref())?);
    let users: Vec<i64> = kernel::users()?.into_iter().map(|user| user.id).collect();
    if users.is_empty() {
        return Ok(());
    }
    let libraries = kernel::libraries()?;
    let named = named_libraries(
        &named,
        &libraries,
        |key| Ok(remembered_ids(kernel::store_get(key, None)?.as_deref())),
        |key, ids| kernel::store_put(key, None, &remembered_value(ids)),
    )?;
    let mut refused = Vec::new();
    for library in named {
        refused.extend(clean(library, &users, cutoff_ms, tick.now_ms)?);
    }
    match refusal(&refused) {
        None => Ok(()),
        Some(refusal) => Err(Error::msg(refusal)),
    }
}

/// Deletes the due titles of `library` at `now_ms`, returning the Kernel's words for each delete it
/// refused.
fn clean(
    library: &Library,
    users: &[i64],
    cutoff_ms: i64,
    now_ms: i64,
) -> Result<Vec<String>, Error> {
    let key = format!("deleting/{}", library.id);
    let recorded = decode(&kernel::store_get(&key, None)?.unwrap_or_default());
    let mut listed = HashSet::new();
    let mut due = Vec::new();
    let mut token = String::new();
    loop {
        let page = kernel::titles_page(library.id, PAGE_SIZE, &token)?;
        let titles: Vec<TitleRef> = (page.titles.iter())
            .filter_map(|title| title.title_ref.as_option().cloned())
            .collect();
        listed.extend(titles.iter().map(Key::of));
        for watched in kernel::watch_states(&titles)? {
            if is_due(&watched, users, cutoff_ms)
                && let Some(title) = watched.title_ref.into_option()
            {
                due.push(title);
            }
        }
        if page.next_page_token.is_empty() {
            break;
        }
        token = page.next_page_token;
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

/// The Library names `setting` lists, separated by commas, trimmed, leaving out empty ones.
fn library_names(setting: &str) -> Vec<&str> {
    (setting.split(','))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

/// The Libraries `named` names among `libraries`, each once and in `libraries`' order,
/// remembering through `put` the ids each name resolved to and reading them back through `get`.
///
/// Names aren't unique, so a name resolves to every Library that has it now. It also keeps the
/// Libraries it last resolved to that still exist, since they were renamed, unless every Library
/// that has it now is new to it: another Library took the name. A name too long for a Store key
/// resolves by name only.
fn named_libraries<'a>(
    named: &[&str],
    libraries: &'a [Library],
    mut get: impl FnMut(&str) -> Result<Vec<i64>, Error>,
    mut put: impl FnMut(&str, &[i64]) -> Result<(), Error>,
) -> Result<Vec<&'a Library>, Error> {
    let mut found = HashSet::new();
    for name in named {
        let key = Some(format!("library/{name}")).filter(|key| key.len() <= MAX_KEY);
        let remembered = match &key {
            Some(key) => get(key)?,
            None => Vec::new(),
        };
        let by_name: Vec<i64> = (libraries.iter())
            .filter(|library| library.name == *name)
            .map(|library| library.id)
            .collect();
        let taken = !by_name.is_empty() && !by_name.iter().any(|id| remembered.contains(id));
        let ids: Vec<i64> = (libraries.iter())
            .map(|library| library.id)
            .filter(|id| by_name.contains(id) || (!taken && remembered.contains(id)))
            .collect();
        if let Some(key) = &key
            && !ids.is_empty()
            && ids != remembered
        {
            put(key, &ids)?;
        }
        found.extend(ids);
    }
    Ok((libraries.iter())
        .filter(|library| found.contains(&library.id))
        .collect())
}

/// The Library ids a Store value [`remembered_value`] made holds, leaving out one it can't read.
fn remembered_ids(value: Option<&[u8]>) -> Vec<i64> {
    (String::from_utf8_lossy(value.unwrap_or_default()).split(','))
        .filter_map(|id| id.parse().ok())
        .collect()
}

/// `ids` as a Store value, separated by commas.
fn remembered_value(ids: &[i64]) -> Vec<u8> {
    let ids: Vec<String> = ids.iter().map(i64::to_string).collect();
    ids.join(",").into_bytes()
}

/// The `days` setting as a number of days, at least 0. A failure names the setting.
fn days(setting: Option<&str>) -> Result<i64, Error> {
    let setting = setting.ok_or_else(|| Error::msg(format!("the setting {DAYS} isn't set")))?;
    match setting.trim().parse::<i64>() {
        Ok(days) if days >= 0 => Ok(days),
        _ => Err(Error::msg(format!(
            "the setting {DAYS} must be a whole number of 0 or more, not {setting}"
        ))),
    }
}

/// The latest time a Watch State may have changed for its title to be due, `days` before `now_ms`.
const fn cutoff_ms(now_ms: i64, days: i64) -> i64 {
    now_ms.saturating_sub(days.saturating_mul(DAY_MS))
}

/// Whether `title` is due: every one of `users` played it no later than `cutoff_ms`, and nobody
/// has it in progress. With no `users`, nothing is due.
fn is_due(title: &TitleWatchStates, users: &[i64], cutoff_ms: i64) -> bool {
    let played_by = |user: i64| {
        (title.states.iter())
            .any(|state| state.user_id == user && state.played && state.updated_at_ms <= cutoff_ms)
    };
    !users.is_empty()
        && !title.states.iter().any(in_progress)
        && users.iter().all(|&user| played_by(user))
}

/// Whether `state` is started and not played, as Continue Watching shows it.
const fn in_progress(state: &WatchState) -> bool {
    state.position_ms > 0 && !state.played
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

/// The tick's error for `refused`, the Kernel's words for each refused delete, each once; `None`
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
            namespace: id.map(|id| id.namespace.clone()).unwrap_or_default(),
            value: id.map(|id| id.value.clone()).unwrap_or_default(),
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

/// A title the tick asked the Kernel to delete, and when.
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
