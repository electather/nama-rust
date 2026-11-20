use std::{cell::RefCell, collections::HashSet};

use nama_pdk::{
    Library, Manifest, Permission, Tick, Title, TitleId, TitleRef, TitleRefKind, TitleRequesters,
    TitleWatchStates, WatchState,
    extism_pdk::Error,
    nama_plugin_proto::nama::plugin::v1::{SettingKind, setting_value::Value},
    testing::{self, with},
};

use super::{
    Asked, Cutoffs, DAY_MS, DAYS, DAYS_UNWATCHED, Deleted, Key, LIBRARIES, MAX_STORED, ask_due,
    chosen_libraries, days, decode, delete_due, encode, manifest, refusal,
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

/// A title the Users `user_ids` Requested.
fn requested_by(user_ids: &[i64]) -> TitleRequesters {
    TitleRequesters {
        title_ref: episode("1").title_ref(4).into(),
        requested: true,
        user_ids: user_ids.to_vec(),
        ..TitleRequesters::default()
    }
}

/// The cutoffs of a 30-day `days` and a 90-day `days_unwatched` at `NOW_MS`.
fn cutoffs() -> Cutoffs {
    Cutoffs::at(NOW_MS, 30, 90)
}

/// Long enough ago for a played title to be due.
const WATCHED_LONG_AGO: i64 = NOW_MS - 31 * DAY_MS;
/// Long enough ago for a title nobody played to be due.
const ADDED_LONG_AGO: i64 = NOW_MS - 91 * DAY_MS;
/// Past `days` but not `days_unwatched`.
const ADDED_A_WHILE_AGO: i64 = NOW_MS - 60 * DAY_MS;

#[test]
fn a_title_its_requester_played_recently_is_kept_however_long_ago_it_was_added() {
    let title = watched(vec![played(ADA, NOW_MS - DAY_MS)]);

    assert!(!cutoffs().is_due(ADDED_LONG_AGO, &requested_by(&[ADA]), &title));
}

#[test]
fn a_title_its_requester_never_finished_is_due_once_days_unwatched_passed_since_it_was_added() {
    let unplayed = WatchState {
        played: false,
        position_ms: 0,
        ..played(ADA, NOW_MS)
    };
    for states in [vec![], vec![started(ADA, NOW_MS)], vec![unplayed]] {
        let title = watched(states);

        assert!(!cutoffs().is_due(ADDED_A_WHILE_AGO, &requested_by(&[ADA]), &title));
        assert!(cutoffs().is_due(ADDED_LONG_AGO, &requested_by(&[ADA]), &title));
    }
}

#[test]
fn a_title_whose_requesters_were_all_deleted_is_due_once_days_unwatched_passed_since_it_was_added()
{
    let title = watched(vec![played(ADA, WATCHED_LONG_AGO)]);

    assert!(!cutoffs().is_due(ADDED_A_WHILE_AGO, &requested_by(&[]), &title));
    assert!(cutoffs().is_due(ADDED_LONG_AGO, &requested_by(&[]), &title));
}

#[test]
fn a_title_played_or_added_exactly_the_days_ago_is_due_and_a_millisecond_later_is_kept() {
    let requesters = requested_by(&[ADA]);
    let played_at = |at_ms| watched(vec![played(ADA, at_ms)]);
    let watched_cutoff = NOW_MS - 30 * DAY_MS;
    let added_cutoff = NOW_MS - 90 * DAY_MS;

    assert!(cutoffs().is_due(NOW_MS, &requesters, &played_at(watched_cutoff)));
    assert!(!cutoffs().is_due(NOW_MS, &requesters, &played_at(watched_cutoff + 1)));
    assert!(cutoffs().is_due(added_cutoff, &requesters, &watched(vec![])));
    assert!(!cutoffs().is_due(added_cutoff + 1, &requesters, &watched(vec![])));
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
fn a_library_chosen_twice_is_cleaned_once() {
    assert_eq!(cleaned(&[4, 4], &[library(4, "Movies")]), [4]);
}

#[test]
fn days_is_a_whole_number_of_zero_or_more() {
    assert_eq!(days("days", Some("30")).ok(), Some(30));
    assert_eq!(days("days", Some("0")).ok(), Some(0));
    for refused in [None, Some("-1"), Some("a week")] {
        let error = days("days_unwatched", refused)
            .map(drop)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("setting days_unwatched"),
            "{refused:?}: {error}"
        );
    }
}

#[test]
fn manifest_declares_days_defaulting_to_30_days_unwatched_to_90_and_libraries() {
    let manifest = manifest();
    let settings: Vec<_> = (manifest.settings.iter())
        .map(|field| {
            let default = field.default_value.as_option();
            (
                field.key.as_str(),
                field.kind.as_known(),
                field.required,
                default.and_then(|default| default.value.clone()),
                (field.min_value, field.max_value),
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
                (Some(0), None),
            ),
            (
                "days_unwatched",
                Some(SettingKind::SETTING_KIND_INTEGER),
                true,
                Some(Value::IntegerValue(90)),
                (Some(0), None),
            ),
            (
                "libraries",
                Some(SettingKind::SETTING_KIND_LIBRARIES),
                false,
                None,
                (None, None),
            ),
        ]
    );
    assert_eq!(manifest.tick_every_seconds, 3600);
    let requested = Manifest {
        permissions: vec![
            Permission::PERMISSION_STORE.into(),
            Permission::PERMISSION_READ_LIBRARIES.into(),
            Permission::PERMISSION_READ_WATCH_STATES.into(),
            Permission::PERMISSION_READ_REQUESTS.into(),
            Permission::PERMISSION_DELETE_MEDIA.into(),
        ],
        ..Manifest::default()
    };
    assert_eq!(manifest.permissions, requested.permissions);
}

/// The Movie `value` in Library 4.
fn movie(value: &str) -> TitleRef {
    TitleRef {
        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
        ..Key {
            namespace: "tmdb".to_owned(),
            ..episode(value)
        }
        .title_ref(4)
    }
}

/// `title_ref` as the Kernel lists it, added at `added_at_ms`.
fn listed(title_ref: &TitleRef, added_at_ms: i64) -> Title {
    Title {
        title_ref: title_ref.clone().into(),
        added_at_ms,
        ..Title::default()
    }
}

/// A Kernel listing `library` with `titles` in Library 4, granting Cleanup its Store, Read
/// Libraries and Read Watch States, and set to clean Library 4 with a 30-day `days` and a 90-day
/// `days_unwatched`. A test grants Read Requests by giving a title's Requesters.
fn listing(library: Library, titles: impl IntoIterator<Item = Title>) -> testing::Kernel {
    testing::Kernel::new()
        .setting(DAYS, "30")
        .setting(DAYS_UNWATCHED, "90")
        .setting(LIBRARIES, "4")
        .store()
        .libraries([library])
        .titles(4, titles)
        // Grants Read Watch States for a test that gives no title a Watch State.
        .watch_states(&movie("unlisted"), [])
}

/// `kernel` granting Delete media too, each delete starting and taking nothing along.
fn deleting(kernel: testing::Kernel) -> testing::Kernel {
    kernel.delete_media(|_title, _also| Ok(Vec::new()))
}

/// [`deleting`] while [`listing`] Library 4 as Movies.
fn cleaning(titles: impl IntoIterator<Item = Title>) -> testing::Kernel {
    deleting(listing(library(4, "Movies"), titles))
}

/// Cleanup's `tick` at `now_ms`, with `kernel` as the Kernel.
fn tick_at(kernel: &mut testing::Kernel, now_ms: i64) -> Result<(), String> {
    let tick = Tick {
        now_ms,
        ..Tick::default()
    };
    with(kernel, || super::tick(&tick)).map_err(|error| error.to_string())
}

/// The titles whose deletes Cleanup asked for, in order, refused or not.
fn asked_to_delete(kernel: &testing::Kernel) -> Vec<TitleRef> {
    (kernel.deletes().iter())
        .map(|delete| delete.title.clone())
        .collect()
}

/// What the Store keeps as asked to delete in Library 4.
fn recorded(kernel: &mut testing::Kernel) -> Vec<Asked> {
    let stored = with(kernel, || nama_pdk::kernel::store_get("deleting/4", None)).unwrap();
    decode(&stored.unwrap_or_default())
}

fn asked_at(title: &TitleRef, at_ms: i64) -> Asked {
    Asked {
        title: Key::of(title),
        at_ms,
    }
}

/// Asserts Cleanup asked to delete `titles`, in order, and its Store keeps each as asked at
/// `NOW_MS`.
fn assert_deleted_at_now(kernel: &mut testing::Kernel, titles: &[TitleRef]) {
    assert_eq!(asked_to_delete(kernel), titles);
    let asked: Vec<Asked> = titles.iter().map(|title| asked_at(title, NOW_MS)).collect();
    assert_eq!(recorded(kernel), asked);
}

#[test]
fn a_movie_its_requester_watched_past_the_days_is_deleted_and_a_recent_one_kept() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_A_WHILE_AGO),
        listed(&dune, ADDED_A_WHILE_AGO),
    ])
    .requesters(alien.clone(), [ADA])
    .requesters(dune.clone(), [ADA])
    .watch_states(&alien, [played(ADA, WATCHED_LONG_AGO)])
    .watch_states(&dune, [played(ADA, NOW_MS - DAY_MS)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[alien]);
}

/// The Kernel fills in a setting not stored with its Manifest's default, so a Cleanup set up
/// before `days_unwatched` existed is given its default beside the `days` it stored.
#[test]
fn a_days_stored_before_days_unwatched_existed_still_applies() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_A_WHILE_AGO),
        listed(&dune, ADDED_A_WHILE_AGO),
    ])
    .setting(DAYS, "10")
    .requesters(alien.clone(), [ADA])
    .requesters(dune.clone(), [ADA])
    .watch_states(&alien, [played(ADA, NOW_MS - 11 * DAY_MS)])
    .watch_states(&dune, [played(ADA, NOW_MS - 9 * DAY_MS)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[alien]);
}

#[test]
fn a_renamed_library_is_still_cleaned() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let dune_in_7 = TitleRef {
        library_id: 7,
        ..dune
    };
    let mut kernel = deleting(listing(
        library(4, "Films"),
        [listed(&alien, ADDED_LONG_AGO)],
    ))
    .libraries([library(7, "Movies")])
    .titles(7, [listed(&dune_in_7, ADDED_LONG_AGO)])
    .requesters(alien.clone(), [ADA])
    .requesters(dune_in_7, [ADA]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[alien]);
}

/// A tick keeps nothing for the next but its Store, so a Library renamed after a tick found it is
/// one the next tick finds renamed. The first tick stored nothing, so the next starts from an
/// empty Store too.
#[test]
fn a_library_renamed_after_a_tick_found_it_is_still_cleaned() {
    let alien = movie("348");
    let mut found = cleaning([listed(&alien, ADDED_A_WHILE_AGO)]).requesters(alien.clone(), [ADA]);
    let nothing_due = tick_at(&mut found, NOW_MS);
    let mut renamed = deleting(listing(
        library(4, "Films"),
        [listed(&alien, ADDED_A_WHILE_AGO)],
    ))
    .requesters(alien.clone(), [ADA]);

    let ticked = tick_at(&mut renamed, NOW_MS + 31 * DAY_MS);

    assert_eq!((nothing_due, asked_to_delete(&found)), (Ok(()), vec![]));
    assert_eq!(recorded(&mut found), []);
    assert_eq!(ticked, Ok(()));
    assert_eq!(
        recorded(&mut renamed),
        [asked_at(&alien, NOW_MS + 31 * DAY_MS)]
    );
    assert_eq!(asked_to_delete(&renamed), [alien]);
}

#[test]
fn without_the_delete_media_permission_nothing_is_deleted_and_the_tick_says_why() {
    let alien = movie("348");
    let mut kernel = listing(library(4, "Movies"), [listed(&alien, ADDED_LONG_AGO)])
        .requesters(alien.clone(), [ADA]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(
        ticked,
        Err(
            "the Kernel refused 1 of the deletes: the Admin hasn't granted the Plugin the Delete \
             media Permission"
                .to_owned()
        )
    );
    assert_eq!(asked_to_delete(&kernel), []);
    assert_eq!(recorded(&mut kernel), []);
}

#[test]
fn a_title_another_user_watched_is_kept_until_its_requester_had_the_days_unwatched() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_A_WHILE_AGO),
        listed(&dune, ADDED_LONG_AGO),
    ])
    .requesters(alien.clone(), [ADA])
    .requesters(dune.clone(), [ADA])
    .watch_states(&alien, [played(BEN, WATCHED_LONG_AGO)])
    .watch_states(&dune, [played(BEN, WATCHED_LONG_AGO)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[dune]);
}

#[test]
fn a_title_two_users_requested_is_kept_until_both_are_done() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_A_WHILE_AGO),
        listed(&dune, ADDED_A_WHILE_AGO),
    ])
    .requesters(alien.clone(), [ADA, BEN])
    .requesters(dune.clone(), [ADA, BEN])
    .watch_states(&alien, [played(ADA, WATCHED_LONG_AGO), played(BEN, NOW_MS)])
    .watch_states(
        &dune,
        [played(ADA, WATCHED_LONG_AGO), played(BEN, WATCHED_LONG_AGO)],
    );

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[dune]);
}

#[test]
fn a_title_its_requester_never_played_is_deleted_once_the_days_unwatched_passed() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_LONG_AGO),
        listed(&dune, ADDED_A_WHILE_AGO),
    ])
    .requesters(alien.clone(), [ADA])
    .requesters(dune.clone(), [ADA]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[alien]);
}

#[test]
fn a_title_nobody_requested_is_never_deleted() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_LONG_AGO),
        listed(&dune, ADDED_LONG_AGO),
    ])
    .requesters(dune.clone(), [ADA])
    .watch_states(&alien, [played(ADA, WATCHED_LONG_AGO)])
    .watch_states(&dune, [played(ADA, WATCHED_LONG_AGO)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[dune]);
}

#[test]
fn a_title_whose_requesters_were_all_deleted_is_deleted_once_the_days_unwatched_passed() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_LONG_AGO),
        listed(&dune, ADDED_A_WHILE_AGO),
    ])
    .requesters(alien.clone(), [])
    .requesters(dune.clone(), [])
    .watch_states(&alien, [played(ADA, WATCHED_LONG_AGO)])
    .watch_states(&dune, [played(ADA, WATCHED_LONG_AGO)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[alien]);
}

/// A played title stays played when started again, so a Requester starting it again unmarks it
/// first and then has it started, not played.
#[test]
fn a_title_its_requester_started_again_is_kept() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let mut kernel = cleaning([
        listed(&alien, ADDED_A_WHILE_AGO),
        listed(&dune, ADDED_A_WHILE_AGO),
    ])
    .requesters(alien.clone(), [ADA])
    .requesters(dune.clone(), [ADA])
    .watch_states(&alien, [started(ADA, NOW_MS)])
    .watch_states(&dune, [played(ADA, WATCHED_LONG_AGO)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[dune]);
}

#[test]
fn an_episode_of_a_followed_show_waits_for_its_requesters() {
    const ADMIN: i64 = 3;
    let (secrets, lies) = (episode("1").title_ref(4), episode("2").title_ref(4));
    let mut kernel = cleaning([
        listed(&secrets, ADDED_A_WHILE_AGO),
        listed(&lies, ADDED_A_WHILE_AGO),
    ])
    .requesters(secrets.clone(), [ADA])
    .requesters(lies.clone(), [ADA])
    .watch_states(&secrets, [played(ADA, WATCHED_LONG_AGO)])
    .watch_states(&lies, [played(ADMIN, WATCHED_LONG_AGO)]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[secrets]);
}

/// A delete runs in the background, so a title still listed after one was asked had its delete
/// fail.
#[test]
fn a_failed_delete_is_asked_again_a_day_later() {
    let alien = movie("348");
    let mut kernel = cleaning([listed(&alien, ADDED_LONG_AGO)]).requesters(alien.clone(), [ADA]);

    let ticks = [NOW_MS, NOW_MS + HOUR_MS, NOW_MS + DAY_MS].map(|now_ms| {
        let ticked = tick_at(&mut kernel, now_ms);
        (ticked, kernel.deletes().len())
    });

    assert_eq!(ticks, [(Ok(()), 1), (Ok(()), 1), (Ok(()), 2)]);
    assert_eq!(recorded(&mut kernel), [asked_at(&alien, NOW_MS + DAY_MS)]);
}

#[test]
fn titles_over_more_than_one_page_are_all_cleaned() {
    let movies: Vec<TitleRef> = (0..=super::PAGE_SIZE)
        .map(|n| movie(&n.to_string()))
        .collect();
    let (first, last) = (movies[0].clone(), movies[movies.len() - 1].clone());
    let mut kernel = cleaning(movies.iter().map(|movie| listed(movie, ADDED_LONG_AGO)))
        .requesters(first.clone(), [ADA])
        .requesters(last.clone(), [ADA]);

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(ticked, Ok(()));
    assert_deleted_at_now(&mut kernel, &[first, last]);
}

#[test]
fn an_episode_a_delete_took_along_is_recorded_and_not_asked_again() {
    let (secrets, lies) = (episode("1").title_ref(4), episode("2").title_ref(4));
    let taken_along = lies.clone();
    let mut kernel = listing(
        library(4, "Movies"),
        [
            listed(&secrets, ADDED_LONG_AGO),
            listed(&lies, ADDED_LONG_AGO),
        ],
    )
    .requesters(secrets.clone(), [ADA])
    .requesters(lies.clone(), [ADA])
    .delete_media(move |_title, _also| Ok(vec![taken_along.clone()]));

    let ticks = [NOW_MS, NOW_MS + HOUR_MS].map(|now_ms| tick_at(&mut kernel, now_ms));

    assert_eq!(ticks, [Ok(()), Ok(())]);
    let asked: Vec<_> = (kernel.deletes().iter())
        .map(|delete| (delete.title.clone(), delete.also.clone()))
        .collect();
    assert_eq!(asked, [(secrets.clone(), vec![lies.clone()])]);
    assert_eq!(
        recorded(&mut kernel),
        [asked_at(&secrets, NOW_MS), asked_at(&lies, NOW_MS)]
    );
}

#[test]
fn a_refused_delete_is_collected_while_the_rest_go_on() {
    let (alien, dune) = (movie("348"), movie("438631"));
    let refused = alien.clone();
    let mut kernel = listing(
        library(4, "Movies"),
        [
            listed(&alien, ADDED_LONG_AGO),
            listed(&dune, ADDED_LONG_AGO),
        ],
    )
    .requesters(alien.clone(), [ADA])
    .requesters(dune.clone(), [ADA])
    .delete_media(move |title, _also| {
        if *title == refused {
            Err("media in /movies can't be deleted: the Server can't write to it".to_owned())
        } else {
            Ok(Vec::new())
        }
    });

    let ticked = tick_at(&mut kernel, NOW_MS);

    assert_eq!(
        ticked,
        Err(
            "the Kernel refused 1 of the deletes: media in /movies can't be deleted: the Server \
             can't write to it"
                .to_owned()
        )
    );
    assert_eq!(asked_to_delete(&kernel), [alien, dune.clone()]);
    assert_eq!(recorded(&mut kernel), [asked_at(&dune, NOW_MS)]);
}
