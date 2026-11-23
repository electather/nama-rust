//! Listing a Library's Unmatched Items through the generated `MatchService` and `BrowseService`
//! clients, over a real socket: which Library, its pages, and what a User without Permissions
//! sees, plans, plays and has Matched.

#[expect(dead_code, reason = "this binary uses only some of the helpers")]
#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ErrorCode, client::HttpClient};
use http::fetch;
use nama_proto::nama::v1::{
    AudioCodec, BrowseServiceClient, Container, DeviceDecoding, ExternalId, GetMovieRequest,
    GetUnmatchedVersionRequest, LibraryKind, ListUnmatchedItemsRequest, ListUnmatchedRequest,
    MatchServiceClient, PageRequest, PlanPlaybackRequest, PlaybackMethod, PlaybackServiceClient,
    RefusalReason, ReportProgressRequest, SetMatchRequest, TitleKind, UnmatchedItemFilter,
    VideoCodec, WatchStateServiceClient,
};
use refused::assert_refused;
use server::TestServer;

/// `ListUnmatchedItems`' `page` of the Library `library`'s items.
fn in_library(library: i64, page: PageRequest) -> ListUnmatchedItemsRequest {
    let filter = UnmatchedItemFilter {
        library_id: library,
        ..Default::default()
    };
    ListUnmatchedItemsRequest {
        filter: filter.into(),
        page: page.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn listing_the_unmatched_items_of_a_library_that_does_not_exist_is_not_found() {
    let server = TestServer::builder().start().await;
    let films = server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());

    let refused = matches
        .list_unmatched_items(in_library(films.id + 1, PageRequest::default()))
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_page_token_reused_for_another_library_is_refused() {
    let server = TestServer::builder().start().await;
    let movies = LibraryKind::LIBRARY_KIND_MOVIES;
    // No title source, so both stay Unmatched.
    let films = (server.matched_library(movies, &["Alien (1979).mkv", "Heat (1995).mkv"])).await;
    let other = server.create_library(movies, &[]).await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let one = PageRequest {
        size: 1,
        ..Default::default()
    };
    let first = matches
        .list_unmatched_items(in_library(films.id, one))
        .await;
    let token = first
        .unwrap()
        .into_owned()
        .page
        .into_option()
        .unwrap()
        .next_token;

    let carried_over = PageRequest {
        token,
        ..Default::default()
    };
    let refused = matches
        .list_unmatched_items(in_library(other.id, carried_over))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
    );
    assert_eq!(refusal.field, "page.token");
}

#[tokio::test]
async fn files_whose_names_do_not_parse_come_on_the_first_page_only() {
    let server = TestServer::builder().start().await;
    let names = ["Alien (1979).mkv", "Heat (1995).mkv", "home video.mkv"];
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &names)).await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let one = PageRequest {
        size: 1,
        ..Default::default()
    };
    let first = matches
        .list_unmatched_items(in_library(films.id, one))
        .await;
    let first = first.unwrap().into_owned();

    let next = PageRequest {
        token: first.page.next_token.clone(),
        ..Default::default()
    };
    let second = matches
        .list_unmatched_items(in_library(films.id, next))
        .await;

    assert_eq!(first.unparsed_paths, [films.path("home video.mkv")]);
    assert_eq!(
        second.unwrap().into_owned().unparsed_paths,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_movie_named_without_a_year_that_no_title_fits_is_listed_by_its_title() {
    let server = TestServer::builder().tmdb().start().await;
    let home_movies = "Home Movies/Home Movies.mkv";
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[home_movies])).await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());

    let listed = matches
        .list_unmatched_items(in_library(films.id, PageRequest::default()))
        .await;

    let listed = listed.unwrap().into_owned();
    let names: Vec<_> = (listed.items.iter())
        .map(|item| (item.title.as_str(), item.year))
        .collect();
    assert_eq!(names, [("Home Movies", None)]);
    assert_eq!(listed.unparsed_paths, Vec::<String>::new());
}

#[tokio::test]
async fn a_user_without_permissions_lists_unmatched_items_by_parsed_name_newest_first() {
    let server = TestServer::builder().start().await;
    let names = ["Alien (1979).mkv", "Heat (1995).mkv", "home video.mkv"];
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &names)).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());

    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    // Files found in one second tie on when, so the scan's directory order decides which is
    // newer: the higher id.
    let mut by_id: Vec<_> = (listed.items.iter())
        .map(|item| {
            (
                item.version_id,
                item.kind.as_known(),
                item.title.as_str(),
                item.year,
            )
        })
        .collect();
    assert!(by_id.iter().all(|&(id, ..)| id > 0));
    assert!(by_id.is_sorted_by(|a, b| a.0 > b.0));
    by_id.sort_by_key(|&(_, _, title, _)| title);
    let parsed: Vec<_> = by_id
        .iter()
        .map(|&(_, kind, title, year)| (kind, title, year))
        .collect();
    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    assert_eq!(
        parsed,
        [(movie, "Alien", Some(1979)), (movie, "Heat", Some(1995))]
    );
    assert_eq!(listed.page.total_size, Some(2));
}

#[tokio::test]
async fn listing_the_unmatched_items_of_no_library_as_a_user_is_not_found() {
    let server = TestServer::builder().start().await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());

    let refused = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: 404,
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_users_page_token_holds_no_path_and_lists_the_next_page() {
    let server = TestServer::builder().start().await;
    let names = ["Alien (1979).mkv", "Heat (1995).mkv"];
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &names)).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let page = |size, token| ListUnmatchedRequest {
        library_id: films.id,
        page: PageRequest {
            size,
            token,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };

    let first = browse.list_unmatched(page(1, String::new())).await;
    let first = first.unwrap().into_owned();
    let token = first.page.into_option().unwrap().next_token;
    let second = browse.list_unmatched(page(0, token.clone())).await;

    let cursor = String::from_utf8(URL_SAFE_NO_PAD.decode(&token).unwrap()).unwrap();
    assert!(!cursor.contains(".mkv"), "{cursor}");
    let second = second.unwrap().into_owned();
    // Which film is newer depends on the scan's directory order, so the pages just split them.
    let mut titles = vec![
        first.items[0].title.as_str(),
        second.items[0].title.as_str(),
    ];
    titles.sort_unstable();
    assert_eq!((second.items.len(), titles), (1, vec!["Alien", "Heat"]));
}

#[tokio::test]
async fn a_user_gets_an_unmatched_items_version_by_its_id_plans_it_and_plays_its_file() {
    let server = TestServer::builder().start().await;
    let films =
        (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &["Alien (1979).mkv"])).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let admins = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let listed = admins
        .list_unmatched_items(in_library(films.id, PageRequest::default()))
        .await
        .unwrap()
        .into_owned();
    let version_id = listed.items[0].version_id;

    let got = browse
        .get_unmatched_version(GetUnmatchedVersionRequest {
            library_id: films.id,
            version_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let playback = PlaybackServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let planned = playback
        .plan_playback(PlanPlaybackRequest {
            version_id,
            device_decoding: chrome().into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let plan = planned.plan.into_option().unwrap();
    let played = fetch(&server, &plan.url, Some(&ripley.token)).await;

    assert_eq!(got.item.title, "Alien");
    assert_eq!(got.item.version_id, version_id);
    assert_eq!(got.version.id, version_id);
    assert_eq!(got.version.file_name, "Alien (1979).mkv");
    assert_eq!(plan.method, PlaybackMethod::PLAYBACK_METHOD_DIRECT_PLAY);
    assert_eq!(played.status, 200);
}

/// What Chrome says it decodes, in part: H.264 and AAC in MP4.
fn chrome() -> DeviceDecoding {
    DeviceDecoding {
        containers: vec![Container::CONTAINER_MP4.into()],
        video_codecs: vec![VideoCodec::VIDEO_CODEC_H264.into()],
        audio_codecs: vec![AudioCodec::AUDIO_CODEC_AAC.into()],
        mse: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn getting_a_version_of_another_library_is_not_found() {
    let server = TestServer::builder().start().await;
    let movies = LibraryKind::LIBRARY_KIND_MOVIES;
    let films = (server.matched_library(movies, &["Alien (1979).mkv"])).await;
    let other = server.create_library(movies, &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config());
    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    let refused = browse
        .get_unmatched_version(GetUnmatchedVersionRequest {
            library_id: other.id,
            version_id: listed.items[0].version_id,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    assert_eq!(refusal.field, "version_id");
}

#[tokio::test]
async fn a_file_whose_name_does_not_parse_is_not_served() {
    let server = TestServer::builder().start().await;
    let films =
        (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &["home video.mkv"])).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    // The scan stores it without an Item, under the only id the Library has given out.
    let played = fetch(&server, "/media/versions/1", Some(&ripley.token)).await;

    assert_eq!(listed.items.len(), 0);
    assert_eq!(played.status, 404);
}

#[tokio::test]
async fn a_users_progress_in_an_unmatched_version_is_saved_once_its_item_is_matched() {
    let server = TestServer::builder().tmdb().start().await;
    // The fake TMDB finds nothing for it.
    let file = "Home Movies (2019).mkv";
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[file])).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let watch_states =
        WatchStateServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let admins = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .items
        .remove(0);
    let got = browse
        .get_unmatched_version(GetUnmatchedVersionRequest {
            library_id: films.id,
            version_id: listed.version_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let report = || {
        watch_states.report_progress(ReportProgressRequest {
            version_id: listed.version_id,
            position_ms: 600_000,
            duration_ms: 6_000_000,
            ..Default::default()
        })
    };
    let heat = ExternalId {
        namespace: "tmdb".to_owned(),
        value: "949".to_owned(),
        ..Default::default()
    };
    let watch_state = || async {
        let movie = browse
            .get_movie(GetMovieRequest {
                library_id: films.id,
                movie_id: heat.clone().into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movie;
        movie.into_option().unwrap().watch_state.into_option()
    };

    report().await.unwrap();
    admins
        .set_match(SetMatchRequest {
            item_id: got.item.item_id,
            title_id: heat.clone().into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let before = watch_state().await;
    report().await.unwrap();
    let after = watch_state().await;

    assert_eq!(listed.item_id, got.item.item_id);
    assert_eq!(before, None);
    assert_eq!(after.map(|state| state.position_ms), Some(600_000));
}
