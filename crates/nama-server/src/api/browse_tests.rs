use std::collections::{BTreeMap, HashMap};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use connectrpc::ErrorCode;
use nama_proto::nama::v1::{Refusal, RefusalReason};

use crate::{
    browse,
    library::{CustomImageUrls, LocalImageUrls},
    page::TokenInvalid,
};

use super::{
    Fields, ImageRef, Movie, Show, ShownImages, artwork_url, movie_to_proto, refused, show_to_proto,
};

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

#[test]
fn a_page_token_from_another_list_is_refused_naming_its_field() {
    let err = refused(Fields::LIBRARY)(browse::Error::PageTokenInvalid(TokenInvalid));

    let debug = err.details.first().and_then(|detail| detail.debug.clone());
    let refusal: Refusal = serde_json::from_value(debug.unwrap()).unwrap();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert_eq!(
        refusal.reason.as_known(),
        Some(RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID)
    );
    assert_eq!(refusal.field, "page.token");
}
