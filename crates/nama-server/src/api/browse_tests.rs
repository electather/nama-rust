use std::collections::{BTreeMap, HashMap};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use crate::library::{CustomImageUrls, LocalImageUrls};

use super::{ImageRef, Movie, Show, ShownImages, artwork_url, movie_to_proto, show_to_proto};

fn logo() -> ImageRef {
    let plugin = PluginId::from_static("fanart");
    let path = "/logo.png".to_owned();
    ImageRef { plugin, path }
}

#[test]
fn a_movie_and_a_show_carry_their_title_logo_url() {
    let images = ShownImages {
        custom: CustomImageUrls::default(),
        local: LocalImageUrls::default(),
        thumbhashes: HashMap::new(),
    };
    let id = ExternalId::tmdb(348);
    let movie = Movie {
        id: id.clone(),
        ids: ExternalIds::from_iter([id.clone()]),
        plugin: "tmdb".to_owned(),
        title: "Alien".to_owned(),
        original_title: None,
        overview: None,
        release_date: None,
        runtime_minutes: None,
        poster: None,
        backdrop: None,
        logo: Some(logo()),
        genres: Vec::new(),
        content_rating: None,
        versions: Vec::new(),
    };
    let show = Show {
        id: id.clone(),
        ids: ExternalIds::from_iter([id]),
        title: "Alien".to_owned(),
        own_poster: None,
        metadata_version: 0,
        backdrop: None,
        logo: Some(logo()),
        overview: None,
        first_air_date: None,
        genres: Vec::new(),
        content_rating: None,
        seasons: BTreeMap::new(),
    };
    let url = Some(artwork_url("fanart", "/logo.png"));

    assert_eq!(movie_to_proto(movie, None, &images).title_logo_url, url);
    assert_eq!(show_to_proto(&show, &images).title_logo_url, url);
}
