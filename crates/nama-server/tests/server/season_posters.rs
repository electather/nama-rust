//! Picking a Season's Poster from its title sources' artwork, over a real socket with the TMDB
//! Plugin enabled against a fake TMDB.

use crate::support::{browsing, http, server};

use std::fs;

use browsing::{Fixture, tmdb_id};
use connectrpc::client::HttpClient;
use http::{Reply, fetch};
use nama_proto::nama::v1::{
    LibraryServiceClient, ListArtworkRequest, ListArtworkResponse, TitleKind,
};
use server::tmdb::DARK;

/// Where Dark's Season 1 Poster is uploaded to and removed from.
fn season_poster() -> String {
    format!("/custom-images/show/tmdb-{DARK}/season/1/poster")
}

/// Dark Season 1's second English Poster in TMDB's artwork list, the one besides its own whose
/// bytes the fake TMDB has.
const SECOND_POSTER: &str = "9kVd2rTSqPzQ1yW4oNcLbXhE3uM.jpg";

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// What `ListArtwork` gives the Admin for Dark's Season 1.
    async fn season_artwork(&self) -> ListArtworkResponse {
        let libraries =
            LibraryServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let listed = libraries.list_artwork(ListArtworkRequest {
            kind: TitleKind::TITLE_KIND_SHOW.into(),
            id: tmdb_id(DARK).into(),
            season: Some(1),
            ..Default::default()
        });
        listed.await.unwrap().into_owned()
    }

    /// Picks the artwork at `url` as Dark Season 1's Poster, as the Admin.
    async fn pick_season_poster(&self, url: &str) -> Reply {
        let url: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
        self.upload(&format!("{}?artwork={url}", season_poster()), &[])
            .await
    }

    /// Dark Season 1's Poster and whether it is a Custom Image, as `GetShow` gives them.
    async fn season_poster(&self) -> (String, bool) {
        let dark = self.get_dark().await;
        let season = &dark.seasons[0];
        (season.poster_url.clone().unwrap(), season.custom_poster)
    }
}

#[tokio::test]
async fn an_admin_picks_a_seasons_poster_from_its_artwork_list_and_it_is_kept_as_a_custom_image() {
    let fx = Fixture::dark().await;
    let (downloaded, _) = fx.season_poster().await;

    let listed = fx.season_artwork().await;
    let picked = fx.pick_season_poster(&listed.poster_urls[1]).await;

    // Only the Season's own Posters, best first: none of the Show's.
    assert_eq!(
        listed.poster_urls,
        [
            "/media/artwork/tmdb/%2F5LoHuHWA4H8jElFlZDvsmU2n63b.jpg".to_owned(),
            format!("/media/artwork/tmdb/%2F{SECOND_POSTER}"),
            "/media/artwork/tmdb/%2FtHvgWB0sh2EYUPNG1SpbVJ5NhGn.jpg".to_owned(),
        ]
    );
    assert_eq!(listed.backdrop_urls, Vec::<String>::new());
    assert_eq!(picked.status, 204);
    let (custom, is_custom) = fx.season_poster().await;
    assert!(is_custom);
    assert!(
        custom.starts_with("/media/custom-images/show/tmdb-70523/season/1/poster/"),
        "{custom}"
    );
    let fixture = format!(
        "{}/../../plugins/tmdb/fixtures/images/{SECOND_POSTER}",
        env!("CARGO_MANIFEST_DIR")
    );
    let served = fetch(&fx.server, &custom, Some(&fx.server.admin_token)).await;
    assert_eq!(served.body, fs::read(fixture).unwrap());

    fx.remove(&season_poster()).await;
    assert_eq!(fx.season_poster().await, (downloaded, false));
}

#[tokio::test]
async fn picking_a_season_poster_the_server_never_offered_is_refused_and_changes_nothing() {
    let fx = Fixture::dark().await;
    let before = fx.season_poster().await;

    let refused = fx
        .pick_season_poster(&format!("/media/artwork/tmdb/%2F{SECOND_POSTER}"))
        .await;

    assert_eq!(refused.status, 404);
    assert_eq!(fx.season_poster().await, before);
}
