use std::{cell::RefCell, collections::HashSet};

use nama_pdk::{
    Library, Manifest, Permission, TitleId, TitleRef, TitleRefKind, TitleWatchStates, WatchState,
    extism_pdk::Error,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
};

use super::{
    Asked, DAY_MS, Deleted, Key, MAX_STORED, ask_due, chosen_libraries, cutoff_ms, days, decode,
    delete_due, encode, is_due, manifest, refusal,
};

const NOW_MS: i64 = 1_800_000_000_000;
const ADA: i64 = 1;
const BEN: i64 = 2;

fn episode(value: &str) -> Key {
    Key::of(&TitleRef {
        library_id: 4,
        kind: TitleRefKind::TITLE_REF_KIND_EPISODE.into(),
        id: TitleId {
            namespace: "tvdb".to_owned(),
            value: value.to_owned(),
            ..TitleId::default()
        }
        .into(),
        ..TitleRef::default()
    })
}

fn played(user_id: i64, updated_at_ms: i64) -> WatchState {
    WatchState {
        user_id,
        played: true,
        position_ms: 7_000_000,
        duration_ms: 7_200_000,
        updated_at_ms,
        ..WatchState::default()
    }
}

fn started(user_id: i64, updated_at_ms: i64) -> WatchState {
    WatchState {
        played: false,
        position_ms: 60_000,
        ..played(user_id, updated_at_ms)
    }
}

fn watched(states: Vec<WatchState>) -> TitleWatchStates {
    TitleWatchStates {
        title_ref: episode("1").title_ref(4).into(),
        states,
        ..TitleWatchStates::default()
    }
}

/// The cutoff of a 30-day setting at `NOW_MS`.
fn thirty_days_ago() -> i64 {
    cutoff_ms(NOW_MS, 30)
}

#[test]
fn a_title_every_user_played_more_than_the_days_ago_is_due() {
    let long_ago = thirty_days_ago() - DAY_MS;
    let title = watched(vec![played(ADA, long_ago), played(BEN, long_ago)]);

    assert!(is_due(&title, &[ADA, BEN], thirty_days_ago()));
}

#[test]
fn a_title_one_user_played_recently_is_kept() {
    let title = watched(vec![
        played(ADA, thirty_days_ago() - DAY_MS),
        played(BEN, NOW_MS - DAY_MS),
    ]);

    assert!(!is_due(&title, &[ADA, BEN], thirty_days_ago()));
}

#[test]
fn a_title_any_user_has_in_progress_is_kept() {
    let long_ago = thirty_days_ago() - DAY_MS;
    // A User the Kernel no longer lists still keeps it.
    let title = watched(vec![
        played(ADA, long_ago),
        played(BEN, long_ago),
        started(3, long_ago),
    ]);

    assert!(!is_due(&title, &[ADA, BEN], thirty_days_ago()));
}

#[test]
fn a_title_one_user_has_not_played_is_kept() {
    let long_ago = thirty_days_ago() - DAY_MS;
    let title = watched(vec![played(ADA, long_ago)]);

    assert!(!is_due(&title, &[ADA, BEN], thirty_days_ago()));
}

#[test]
fn with_no_users_nothing_is_due() {
    let title = watched(vec![played(ADA, thirty_days_ago() - DAY_MS)]);

    assert!(!is_due(&title, &[], thirty_days_ago()));
}

#[test]
fn a_title_played_exactly_the_days_ago_is_due_and_a_millisecond_later_is_kept() {
    let cutoff = cutoff_ms(NOW_MS, 30);

    assert_eq!(cutoff, NOW_MS - 30 * DAY_MS);
    assert!(is_due(&watched(vec![played(ADA, cutoff)]), &[ADA], cutoff));
    assert!(!is_due(
        &watched(vec![played(ADA, cutoff + 1)]),
        &[ADA],
        cutoff
    ));
}

/// A fake `delete_media`: each delete takes along the titles `files` puts in the same file, and
/// one `refused` names is refused.
struct Kernel {
    files: Vec<Vec<&'static str>>,
    refused: Vec<&'static str>,
    asked: RefCell<Vec<(String, Vec<String>)>>,
}

impl Kernel {
    fn delete(&self, title: &TitleRef, also: &[TitleRef]) -> Result<Vec<TitleRef>, Error> {
        let value = Key::of(title).value;
        let also = also.iter().map(|other| Key::of(other).value).collect();
        self.asked.borrow_mut().push((value.clone(), also));
        if self.refused.contains(&value.as_str()) {
            return Err(Error::msg(format!("the Kernel refused {value}")));
        }
        let file = self
            .files
            .iter()
            .find(|file| file.contains(&value.as_str()));
        Ok(file
            .into_iter()
            .flatten()
            .filter(|&&other| other != value)
            .map(|other| episode(other).title_ref(4))
            .collect())
    }
}

fn kernel(files: Vec<Vec<&'static str>>, refused: Vec<&'static str>) -> Kernel {
    Kernel {
        files,
        refused,
        asked: RefCell::new(Vec::new()),
    }
}

fn titles(values: &[&str]) -> Vec<TitleRef> {
    values
        .iter()
        .map(|value| episode(value).title_ref(4))
        .collect()
}

#[test]
fn titles_taken_along_by_an_earlier_delete_are_recorded_and_not_deleted_again() {
    let kernel = kernel(vec![vec!["1", "2"], vec!["3"]], Vec::new());

    let deleted = delete_due(&titles(&["1", "2", "3"]), &HashSet::new(), |title, also| {
        kernel.delete(title, also)
    });

    assert_eq!(
        kernel.asked.into_inner(),
        [
            ("1".to_owned(), vec!["2".to_owned(), "3".to_owned()]),
            ("3".to_owned(), vec!["1".to_owned(), "2".to_owned()]),
        ]
    );
    assert_eq!(
        deleted,
        Deleted {
            recorded: vec![episode("1"), episode("2"), episode("3")],
            refused: Vec::new(),
        }
    );
}

#[test]
fn titles_already_considered_are_skipped() {
    let kernel = kernel(Vec::new(), Vec::new());
    let considered = HashSet::from([episode("1")]);

    let deleted = delete_due(&titles(&["1", "2"]), &considered, |title, also| {
        kernel.delete(title, also)
    });

    assert_eq!(deleted.recorded, [episode("2")]);
    assert_eq!(kernel.asked.into_inner().len(), 1);
}

#[test]
fn a_refused_delete_is_collected_and_the_rest_go_on() {
    let kernel = kernel(Vec::new(), vec!["1"]);

    let deleted = delete_due(&titles(&["1", "2"]), &HashSet::new(), |title, also| {
        kernel.delete(title, also)
    });

    assert_eq!(
        deleted,
        Deleted {
            recorded: vec![episode("2")],
            refused: vec!["the Kernel refused 1".to_owned()],
        }
    );
}

#[test]
fn the_refusal_counts_every_refused_delete_and_says_each_reason_once() {
    let refused = ["no grant", "no grant", "read-only"].map(str::to_owned);

    assert_eq!(refusal(&[]), None);
    assert_eq!(
        refusal(&refused).as_deref(),
        Some("the Kernel refused 3 of the deletes: no grant; read-only")
    );
}

/// `title` as asked to delete at `at_ms`.
fn asked(title: &str, at_ms: i64) -> Asked {
    Asked {
        title: episode(title),
        at_ms,
    }
}

const HOUR_MS: i64 = 60 * 60 * 1000;

/// What [`ask_due`] does at `NOW_MS` with `recorded`, when the Library lists `listed` and `due`
/// are due, with the values of the titles it asked to delete.
fn ask_at_now(recorded: Vec<Asked>, listed: &[&str], due: &[&str]) -> (Vec<Asked>, Vec<String>) {
    let kernel = kernel(Vec::new(), Vec::new());
    let listed = listed.iter().map(|value| episode(value)).collect();

    let asking = ask_due(recorded, &listed, &titles(due), NOW_MS, |title, also| {
        kernel.delete(title, also)
    });

    assert!(asking.refused.is_empty(), "{asking:?}");
    let delete_asked = kernel
        .asked
        .into_inner()
        .into_iter()
        .map(|(value, _)| value);
    (asking.recorded, delete_asked.collect())
}

#[test]
fn a_title_asked_a_day_ago_still_listed_and_due_is_asked_again() {
    let (recorded, delete_asked) = ask_at_now(vec![asked("1", NOW_MS - DAY_MS)], &["1"], &["1"]);

    assert_eq!(delete_asked, ["1"]);
    assert_eq!(recorded, [asked("1", NOW_MS)]);
}

#[test]
fn a_title_asked_an_hour_ago_still_listed_and_due_is_not_asked_again() {
    let (recorded, delete_asked) = ask_at_now(vec![asked("1", NOW_MS - HOUR_MS)], &["1"], &["1"]);

    assert_eq!(delete_asked, Vec::<String>::new());
    assert_eq!(recorded, [asked("1", NOW_MS - HOUR_MS)]);
}

#[test]
fn a_title_asked_a_day_ago_and_no_longer_due_is_forgotten_and_not_asked() {
    let (recorded, delete_asked) = ask_at_now(vec![asked("1", NOW_MS - DAY_MS)], &["1"], &[]);

    assert_eq!(delete_asked, Vec::<String>::new());
    assert_eq!(recorded, []);
}

#[test]
fn a_title_no_longer_listed_is_forgotten() {
    let (recorded, _) = ask_at_now(vec![asked("1", NOW_MS - HOUR_MS)], &["2"], &["2"]);

    assert_eq!(recorded, [asked("2", NOW_MS)]);
}

#[test]
fn the_store_value_reads_back_what_was_written() {
    let recorded = vec![asked("1", NOW_MS), asked("tt 2", NOW_MS - DAY_MS)];

    assert_eq!(decode(&encode(recorded.clone().into_iter())), recorded);
}

#[test]
fn a_store_value_of_the_form_without_when_it_was_asked_reads_as_nothing_recorded() {
    assert_eq!(decode(b"2\ttvdb\t1\n2\ttvdb\t2\n"), []);
}

#[test]
fn a_store_value_too_large_keeps_the_latest_titles() {
    let recorded: Vec<Asked> = (0..10_000).map(|n| asked(&n.to_string(), NOW_MS)).collect();

    let value = encode(recorded.clone().into_iter());
    let kept = decode(&value);

    assert!(value.len() <= MAX_STORED);
    assert!(kept.len() < recorded.len());
    assert_eq!(kept, recorded[recorded.len() - kept.len()..]);
}

fn library(id: i64, name: &str) -> Library {
    Library {
        id,
        name: name.to_owned(),
        ..Library::default()
    }
}

/// The ids of the Libraries among `libraries` that the setting `ids` chooses.
fn cleaned(ids: &[i64], libraries: &[Library]) -> Vec<i64> {
    (chosen_libraries(ids, libraries).into_iter())
        .map(|library| library.id)
        .collect()
}

#[test]
fn the_libraries_whose_ids_are_chosen_are_cleaned_and_others_are_not() {
    let libraries = [
        library(4, "Movies"),
        library(5, "Shows"),
        library(7, "Kids"),
    ];

    assert_eq!(cleaned(&[7, 4], &libraries), [4, 7]);
    assert_eq!(cleaned(&[], &libraries), Vec::<i64>::new());
}

#[test]
fn an_id_the_kernel_does_not_list_is_skipped() {
    let cleaned = cleaned(&[4, 9], &[library(4, "Movies"), library(5, "Shows")]);

    assert_eq!(cleaned, [4]);
}

#[test]
fn a_renamed_library_is_still_cleaned() {
    let before = cleaned(&[4], &[library(4, "Movies")]);
    let after = cleaned(&[4], &[library(4, "Films"), library(7, "Movies")]);

    assert_eq!(before, [4]);
    assert_eq!(after, [4]);
}

#[test]
fn a_library_chosen_twice_is_cleaned_once() {
    assert_eq!(cleaned(&[4, 4], &[library(4, "Movies")]), [4]);
}

#[test]
fn days_is_a_whole_number_of_zero_or_more() {
    assert_eq!(days(Some("30")).ok(), Some(30));
    assert_eq!(days(Some("0")).ok(), Some(0));
    for refused in [None, Some("-1"), Some("a week")] {
        let error = days(refused).map(drop).unwrap_err().to_string();
        assert!(error.contains("setting days"), "{refused:?}: {error}");
    }
}

#[test]
fn manifest_declares_days_defaulting_to_30_and_libraries() {
    let manifest = manifest();
    let settings: Vec<_> = (manifest.settings.iter())
        .map(|field| {
            let default = field.default_value.as_option();
            (
                field.key.as_str(),
                field.kind.as_known(),
                field.required,
                default.and_then(|default| default.value.clone()),
            )
        })
        .collect();

    assert_eq!(
        settings,
        [
            (
                "days",
                Some(SettingKind::SETTING_KIND_INTEGER),
                true,
                Some(Value::IntegerValue(30)),
            ),
            (
                "libraries",
                Some(SettingKind::SETTING_KIND_LIBRARIES),
                false,
                None
            ),
        ]
    );
    assert_eq!(manifest.tick_every_seconds, 3600);
    let requested = Manifest {
        permissions: vec![
            Permission::PERMISSION_STORE.into(),
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_USERS.into(),
            Permission::PERMISSION_DELETE_MEDIA.into(),
        ],
        ..Manifest::default()
    };
    assert_eq!(manifest.permissions, requested.permissions);
}
