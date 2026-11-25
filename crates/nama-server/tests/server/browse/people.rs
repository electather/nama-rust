//! A Person's page: the titles in a Library they have a Role in.

use nama_proto::nama::v1::{GetPersonRequest, GetPersonResponse, person_role};

use super::*;

/// The page of the Person with the TMDB id `tmdb`, as `GetPerson` gives it.
async fn get_person(fx: &Fixture, tmdb: i64) -> Result<GetPersonResponse, ConnectError> {
    let request = GetPersonRequest {
        person_id: tmdb_id(tmdb).into(),
        ..Default::default()
    };
    Ok(fx.browse.get_person(request).await?.into_owned())
}

/// Each title of `roles` as its name and parts.
fn titles_and_parts(roles: &[nama_proto::nama::v1::PersonRole]) -> Vec<(&str, Vec<&str>)> {
    (roles.iter())
        .map(|role| {
            let title = match &role.title {
                Some(person_role::Title::Movie(movie)) => movie.title.as_str(),
                Some(person_role::Title::Show(show)) => show.title.as_str(),
                None => "",
            };
            (title, role.parts.iter().map(String::as_str).collect())
        })
        .collect()
}

#[tokio::test]
async fn a_role_names_its_person_whose_page_lists_the_title_with_their_part() {
    let fx = Fixture::alien().await;
    let alien = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let weaver = alien.cast[1].person_id.as_option().unwrap();

    let page = get_person(&fx, weaver.value.parse().unwrap())
        .await
        .unwrap();

    assert_eq!(weaver, &tmdb_id(10_205));
    assert_eq!(page.name, "Sigourney Weaver");
    assert_eq!(page.headshot_url, alien.cast[1].headshot_url);
    assert_eq!(
        titles_and_parts(&page.cast),
        [("Alien", vec!["Ellen Ripley"])]
    );
    assert!(page.crew.is_empty(), "{:?}", page.crew);
    let Some(person_role::Title::Movie(movie)) = &page.cast[0].title else {
        panic!("Weaver plays in Alien: {:?}", page.cast);
    };
    assert_eq!(movie.library_id, Some(fx.library));
}

#[tokio::test]
async fn a_persons_jobs_on_a_show_are_listed_once_under_crew() {
    let fx = Fixture::shows().await;
    let dark = fx.get_show(fx.library, DARK).await.unwrap();
    let dark = dark.show.into_option().unwrap();
    let odar = dark.crew[0].person_id.as_option().unwrap();

    let page = get_person(&fx, odar.value.parse().unwrap()).await.unwrap();

    assert_eq!(page.name, "Baran bo Odar");
    assert_eq!(
        titles_and_parts(&page.crew),
        [("Dark", vec!["Creator", "Director"])]
    );
}

#[tokio::test]
async fn a_person_in_no_title_on_the_server_is_not_found() {
    let fx = Fixture::alien().await;

    let page = get_person(&fx, 1).await;

    let refusal = assert_refused(
        page,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_PERSON_NOT_FOUND,
    );
    assert_eq!(refusal.field, "person_id");
}
