use nama_pdk::{
    Permission, SharingTitle, SimilarAsked, SimilarSource, TitleId, TitleKind, TitleRole, services,
    testing::{Kernel, with},
};

use super::{GenresPeople, manifest};

const SCOTT: &str = "578";
const WEAVER: &str = "10205";

fn id(value: &str) -> TitleId {
    TitleId {
        namespace: "tmdb".to_owned(),
        value: value.to_owned(),
        ..TitleId::default()
    }
}

fn crew(person: &str, job: &str) -> TitleRole {
    TitleRole {
        person: id(person).into(),
        crew: true,
        part: job.to_owned(),
        ..TitleRole::default()
    }
}

fn cast(person: &str, billed: u32) -> TitleRole {
    TitleRole {
        person: id(person).into(),
        billed,
        ..TitleRole::default()
    }
}

fn genres(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

fn titled(value: &str, roles: Vec<TitleRole>, names: &[&str]) -> SharingTitle {
    SharingTitle {
        ids: vec![id(value)],
        genres: genres(names),
        roles,
        ..SharingTitle::default()
    }
}

/// Alien, directed by Ridley Scott, with Sigourney Weaver billed first.
fn alien() -> SimilarAsked {
    SimilarAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: vec![id("348")],
        genres: genres(&["Horror", "Science Fiction"]),
        roles: vec![cast(WEAVER, 0), crew(SCOTT, "Director")],
        ..SimilarAsked::default()
    }
}

/// What it answers about Alien, by id and weight to two places, when the Kernel finds `sharing`.
fn similar_to_alien(sharing: Vec<SharingTitle>) -> Vec<(String, f32)> {
    let mut kernel = Kernel::new().titles_sharing(sharing);
    let answer = with(&mut kernel, || GenresPeople::similar(alien())).unwrap();
    (answer.titles.iter())
        .map(|title| {
            (
                title.id.value.clone(),
                (title.weight * 100.0).round() / 100.0,
            )
        })
        .collect()
}

#[test]
fn it_asks_the_kernel_for_titles_sharing_the_titles_persons_and_genres() {
    let mut kernel = Kernel::new().titles_sharing([]);

    with(&mut kernel, || GenresPeople::similar(alien())).unwrap();

    let [asked] = kernel.titles_sharing_asked() else {
        panic!("{:?}", kernel.titles_sharing_asked());
    };
    let mut people: Vec<_> = asked.people.iter().map(|id| id.value.as_str()).collect();
    people.sort_unstable();
    assert_eq!(people, [WEAVER, SCOTT]);
    assert_eq!(asked.genres, alien().genres);
    assert_eq!(asked.kind.as_known(), Some(TitleKind::TITLE_KIND_MOVIE));
}

#[test]
fn a_shared_director_weighs_more_than_shared_genres_alone() {
    let blade_runner = titled("78", vec![crew(SCOTT, "Director")], &["Science Fiction"]);
    let the_thing = titled("1091", vec![], &["Horror", "Science Fiction"]);

    let similar = similar_to_alien(vec![the_thing, blade_runner]);

    // Scott's 0.3 and half the genres' 0.1, then every genre's 0.2.
    assert_eq!(similar, [("78".to_owned(), 0.4), ("1091".to_owned(), 0.2)]);
}

#[test]
fn shared_cast_weighs_less_the_further_down_the_billing() {
    let aliens = titled("679", vec![cast(WEAVER, 0)], &[]);
    let galaxy_quest = titled("926", vec![cast(WEAVER, 4)], &[]);
    let cameo = titled("1", vec![cast(WEAVER, 10)], &[]);

    let similar = similar_to_alien(vec![galaxy_quest, cameo, aliens]);

    assert_eq!(
        similar,
        [
            ("679".to_owned(), 0.1),
            ("926".to_owned(), 0.06),
            ("1".to_owned(), 0.0)
        ]
    );
}

#[test]
fn a_title_is_kept_only_sharing_a_person_or_half_its_genres() {
    let a_third = titled("1", vec![], &["Horror", "Comedy", "Romance"]);
    let a_half = titled("2", vec![], &["Horror", "Comedy"]);

    let similar = similar_to_alien(vec![a_third, a_half]);

    let ids: Vec<_> = similar.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["2"]);
}

#[test]
fn a_weight_stops_under_a_title_sources_top_pick() {
    // Four shared writers alone would weigh 1.2.
    let writers = ["1", "2", "3", "4"].map(|person| crew(person, "Writer"));
    let asked = SimilarAsked {
        roles: writers.to_vec(),
        ..alien()
    };
    let mut kernel = Kernel::new().titles_sharing([titled("1", writers.to_vec(), &[])]);

    let answer = with(&mut kernel, || GenresPeople::similar(asked)).unwrap();

    assert_eq!(answer.titles[0].weight, 0.9);
}

#[test]
fn the_title_asked_about_is_not_its_own_similar_title() {
    let alien_itself = titled("348", vec![crew(SCOTT, "Director")], &["Horror"]);

    assert_eq!(similar_to_alien(vec![alien_itself]), []);
}

#[test]
fn without_read_libraries_it_fails_with_the_kernels_words() {
    let mut kernel = Kernel::new();

    let failed = with(&mut kernel, || GenresPeople::similar(alien())).unwrap_err();

    assert!(failed.to_string().contains("Read Libraries"), "{failed}");
}

#[test]
fn manifest_is_a_similar_source_reading_libraries_only_without_settings() {
    let manifest = manifest();

    let points: Vec<_> = manifest.points.iter().map(|point| &point.service).collect();
    assert_eq!(points, [services::SIMILAR_SOURCE]);
    assert_eq!(
        manifest.permissions,
        [Permission::PERMISSION_READ_LIBRARIES]
    );
    assert_eq!(manifest.settings, []);
}
