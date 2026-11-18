use nama_plugin_api::metadata::ExternalIds;

use super::*;

#[test]
fn a_show_and_a_movie_that_share_an_id_each_get_only_their_own_images() {
    let shared = ExternalId::new("tmdb", "1".to_owned()).unwrap();
    let ids: ExternalIds = iter::once(shared.clone()).collect();
    let image = |role, season, stored: &str| TitleLocalImage {
        ids: ids.clone(),
        role,
        season,
        library: 1,
        stored: stored.to_owned(),
    };
    let poster = LocalRole::Title(Role::Poster);

    let urls = LocalImageUrls::filed(vec![
        image(poster, None, "movie"),
        image(poster, Some(1), "show"),
        image(LocalRole::SeasonPoster, Some(1), "season"),
    ]);

    let shown = (
        urls.of_movie(&shared, &ids, Role::Poster),
        urls.of_show(&shared, &ids, Role::Poster),
        urls.of_season(&shared, &ids, 1),
        urls.of_season(&shared, &ids, 2),
    );
    let url = |stored| Some(format!("/media/local-images/1/{stored}"));
    assert_eq!(shown, (url("movie"), url("show"), url("season"), None));
}
