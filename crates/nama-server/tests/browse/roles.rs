//! A Movie's and a Show's Cast and Crew, and the headshots served for them.

use nama_proto::nama::v1::Role;

use super::*;

/// Each of `roles` as its name and part.
fn parts(roles: &[Role]) -> Vec<(&str, &str)> {
    (roles.iter())
        .map(|role| (role.name.as_str(), role.part.as_str()))
        .collect()
}

#[tokio::test]
async fn a_movie_has_its_cast_and_crew_and_its_casts_headshots_are_served() {
    let fx = Fixture::alien().await;

    let alien = fx.get_movie(fx.library, ALIEN).await.unwrap();
    let headshot = alien.cast[1].headshot_url.clone().unwrap();
    let reply = fetch(&fx.server, &headshot, Some(&fx.server.admin_token)).await;

    let cast = [
        ("Tom Skerritt", "Dallas"),
        ("Sigourney Weaver", "Ellen Ripley"),
    ];
    assert_eq!(parts(&alien.cast), cast);
    let crew = [("Ridley Scott", "Director"), ("Dan O'Bannon", "Screenplay")];
    assert_eq!(parts(&alien.crew), crew);
    assert_eq!(alien.crew[0].headshot_url, None);
    assert_eq!(reply.status, 200);
}

#[tokio::test]
async fn a_show_has_its_cast_and_crew_with_its_creators_first() {
    let fx = Fixture::shows().await;

    let dark = fx.get_show(fx.library, DARK).await.unwrap();
    let dark = dark.show.into_option().unwrap();

    let cast = [
        ("Louis Hofmann", "Jonas Kahnwald"),
        ("Lisa Vicari", "Martha Nielsen"),
    ];
    assert_eq!(parts(&dark.cast), cast);
    let crew = [
        ("Baran bo Odar", "Creator"),
        ("Jantje Friese", "Creator"),
        ("Baran bo Odar", "Director"),
    ];
    assert_eq!(parts(&dark.crew), crew);
    assert_eq!(
        dark.cast[0].headshot_url.as_deref(),
        Some("/media/artwork/tmdb/%2Fhofmann.jpg")
    );
}
