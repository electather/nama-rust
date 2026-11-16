use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use nama_pdk::{
    Library, Manifest, Permission, TitleId, TitleRef, TitleRefKind, TitleWatchStates, WatchState,
    extism_pdk::Error,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
};

use super::{
    DAY_MS, Deleted, Key, MAX_STORED, cutoff_ms, days, decode, delete_due, encode, is_due,
    library_names, manifest, named_libraries, refusal, remembered_ids, remembered_value,
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

#[test]
fn the_store_value_reads_back_what_was_written() {
    let recorded = vec![episode("1"), episode("tt 2")];

    assert_eq!(decode(&encode(recorded.clone().into_iter())), recorded);
}

#[test]
fn a_store_value_too_large_keeps_the_latest_titles() {
    let recorded: Vec<Key> = (0..10_000).map(|n| episode(&n.to_string())).collect();

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

/// A fake Store, keeping what [`named_libraries`] remembers as the Kernel's Store would.
#[derive(Default)]
struct Store(RefCell<HashMap<String, Vec<u8>>>);

impl Store {
    /// The ids of the Libraries `named` cleans among `libraries`, remembering in this Store.
    fn cleaned(&self, named: &[&str], libraries: &[Library]) -> Vec<i64> {
        let found = named_libraries(
            named,
            libraries,
            |key| Ok(remembered_ids(self.0.borrow().get(key).map(Vec::as_slice))),
            |key, ids| {
                (self.0.borrow_mut()).insert(key.to_owned(), remembered_value(ids));
                Ok(())
            },
        )
        .unwrap();
        found.into_iter().map(|library| library.id).collect()
    }
}

#[test]
fn a_named_library_is_cleaned_and_others_are_not() {
    let store = Store::default();

    let cleaned = store.cleaned(&["Movies"], &[library(4, "Movies"), library(5, "Shows")]);

    assert_eq!(cleaned, [4]);
}

#[test]
fn a_renamed_library_is_still_cleaned_under_its_old_name() {
    let store = Store::default();
    store.cleaned(&["Movies"], &[library(4, "Movies")]);

    let cleaned = store.cleaned(&["Movies"], &[library(4, "Films")]);

    assert_eq!(cleaned, [4]);
}

#[test]
fn a_name_a_new_library_takes_cleans_the_new_one() {
    let store = Store::default();
    store.cleaned(&["Movies"], &[library(4, "Movies")]);
    store.cleaned(&["Movies"], &[library(4, "Films")]);

    let taken = store.cleaned(&["Movies"], &[library(4, "Films"), library(7, "Movies")]);
    let renamed_again = store.cleaned(&["Movies"], &[library(4, "Films"), library(7, "Kids")]);

    assert_eq!(taken, [7]);
    assert_eq!(renamed_again, [7]);
}

#[test]
fn libraries_sharing_a_name_are_all_cleaned_and_still_are_after_one_is_renamed() {
    let store = Store::default();
    let cleaned = store.cleaned(&["Movies"], &[library(4, "Movies"), library(5, "Movies")]);

    let renamed = store.cleaned(
        &["Movies"],
        &[library(4, "Movies"), library(5, "Movies 4K")],
    );
    let both_renamed = store.cleaned(&["Movies"], &[library(4, "Films"), library(5, "Movies 4K")]);

    assert_eq!(cleaned, [4, 5]);
    assert_eq!(renamed, [4, 5]);
    assert_eq!(both_renamed, [4, 5]);
}

#[test]
fn a_new_library_taking_a_shared_name_joins_the_ones_still_having_it() {
    let store = Store::default();
    store.cleaned(&["Movies"], &[library(4, "Movies"), library(5, "Movies")]);

    let cleaned = store.cleaned(
        &["Movies"],
        &[
            library(4, "Movies"),
            library(5, "Films"),
            library(7, "Movies"),
        ],
    );

    assert_eq!(cleaned, [4, 5, 7]);
}

#[test]
fn remembered_ids_read_back_what_was_put() {
    assert_eq!(remembered_ids(Some(&remembered_value(&[4, 5]))), [4, 5]);
    assert_eq!(remembered_ids(Some(b"4,x,5")), [4, 5]);
    assert_eq!(remembered_ids(None), Vec::<i64>::new());
}

#[test]
fn a_name_whose_library_is_deleted_cleans_nothing() {
    let store = Store::default();
    store.cleaned(&["Movies"], &[library(4, "Movies")]);

    let cleaned = store.cleaned(&["Movies"], &[library(5, "Shows")]);

    assert_eq!(cleaned, Vec::<i64>::new());
}

#[test]
fn a_library_named_twice_is_cleaned_once() {
    let store = Store::default();
    store.cleaned(&["Movies"], &[library(4, "Movies")]);

    let cleaned = store.cleaned(&["Movies", "Films"], &[library(4, "Films")]);

    assert_eq!(cleaned, [4]);
}

#[test]
fn a_name_too_long_for_a_store_key_is_matched_by_name_only() {
    let store = Store::default();
    let long = "M".repeat(300);
    store.cleaned(&[&long], &[library(4, &long)]);

    let renamed = store.cleaned(&[&long], &[library(4, "Films")]);

    assert!(store.0.borrow().is_empty());
    assert_eq!(renamed, Vec::<i64>::new());
}

#[test]
fn library_names_are_split_on_commas_and_trimmed() {
    assert_eq!(library_names(" Movies , Kids TV,,"), ["Movies", "Kids TV"]);
    assert_eq!(library_names(" , "), Vec::<&str>::new());
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
                Some(SettingKind::SETTING_KIND_TEXT),
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
