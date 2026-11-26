//! A User's Maximum Age keeps other Users' Requests and Followed Shows for titles over it from
//! their Request list, a Request read by id and their Events, over a real socket with the fake
//! TMDB, which rates Alien R (17) and Dark TV-MA (17) and gives Heat and Dune no US rating. Their
//! own Requests reach them whatever their rating.

use crate::support::server;

use std::collections::BTreeSet;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    EventType, GetRequestRequest, ListRequestsRequest, MaximumAge, Request, RequestServiceClient,
    SetMaximumAgeRequest, UserServiceClient, event, request::MovieOrSeason,
};
use server::{
    SignedIn, Subscription, TestServer,
    requests::Requester,
    subscribe,
    tmdb::{ALIEN, DARK, DUNE, HEAT},
};

/// A Server where each viewer asked for Dune before getting their Maximum Age, and Hicks, who has
/// none, asks for the rest.
struct Fixture {
    server: TestServer,
    hicks: SignedIn,
    /// Each viewer, with the Maximum Age a test gives them, as an age and whether unrated titles
    /// reach them, or `None` for none.
    viewers: Vec<(SignedIn, Option<(u32, bool)>)>,
    /// The id of Dune's Request.
    dune: i64,
}

impl Fixture {
    async fn start() -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let hicks = server.user_with_default_permissions("Hicks").await;
        let caps = [None, Some((17, false)), Some((16, true)), Some((16, false))];
        let mut viewers = Vec::new();
        let mut dune = 0;
        for (n, cap) in caps.into_iter().enumerate() {
            let viewer = server
                .user_with_default_permissions(&format!("Viewer {n}"))
                .await;
            dune = server.requests(&viewer).ask(DUNE).await.id;
            if let Some((age, allow_unrated)) = cap {
                set_maximum_age(&server, viewer.id, age, allow_unrated).await;
            }
            viewers.push((viewer, cap));
        }
        Self {
            server,
            hicks,
            viewers,
            dune,
        }
    }

    /// Hicks asks for Alien, follows Dark and asks for Heat, and the Admin declines Alien.
    async fn hicks_asks(&self) {
        let hicks = self.server.requests(&self.hicks);
        let alien = hicks.ask(ALIEN).await;
        hicks.follow(DARK).await;
        hicks.ask(HEAT).await;
        self.server.decline(alien.id, "").await;
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn set_maximum_age(server: &TestServer, user: i64, age: u32, allow_unrated: bool) {
    let users = UserServiceClient::new(HttpClient::plaintext(), server.client_config());
    let request = SetMaximumAgeRequest {
        user_id: user,
        maximum_age: MaximumAge {
            age,
            allow_unrated,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    users.set_maximum_age(request).await.unwrap();
}

/// The title a Request asks for: the Movie's, or the Show's for a Season.
fn title_of(request: &Request) -> String {
    match &request.movie_or_season {
        Some(MovieOrSeason::Movie(movie)) => movie.title.clone(),
        Some(MovieOrSeason::Season(season)) => season.show_title.clone(),
        None => String::new(),
    }
}

/// What Hicks's asking leaves in each viewer's list: the titles of its Requests and Followed
/// Shows, and the total it gives.
type Listed = (BTreeSet<String>, Option<i32>);

#[tokio::test]
async fn a_capped_user_lists_others_requests_only_for_titles_their_maximum_age_lets_through() {
    let fx = Fixture::start().await;
    fx.hicks_asks().await;

    let mut listed: Vec<Listed> = Vec::new();
    for (viewer, _) in &fx.viewers {
        let client = fx.server.requests(viewer);
        let page = client.list_requests(ListRequestsRequest::default());
        let page = page.await.unwrap().into_owned();
        let requests = page.requests.iter().map(title_of);
        let followed =
            (page.followed_shows.iter()).map(|show| format!("following {}", show.show_title));
        let titles = requests.chain(followed).collect();
        listed.push((titles, page.page.total_size));
    }

    let titles = |names: &[&str]| names.iter().map(|&name| name.to_owned()).collect();
    // Alien, Dark's three Season Requests and Followed Show, Heat and Dune.
    assert_eq!(
        listed,
        [
            (
                titles(&["Alien", "Dark", "following Dark", "Heat", "Dune"]),
                Some(7)
            ),
            (
                titles(&["Alien", "Dark", "following Dark", "Dune"]),
                Some(6)
            ),
            (titles(&["Heat", "Dune"]), Some(2)),
            (titles(&["Dune"]), Some(1)),
        ]
    );
}

#[tokio::test]
async fn a_request_over_a_capped_users_age_is_not_found_by_id_unless_theirs() {
    let fx = Fixture::start().await;
    let alien = fx.server.requests(&fx.hicks).ask(ALIEN).await.id;
    let get = async |viewer: &SignedIn, id| {
        let request = GetRequestRequest {
            id,
            ..Default::default()
        };
        let client = RequestServiceClient::new(HttpClient::plaintext(), viewer.client_config());
        client.get_request(request).await.map(drop)
    };
    let (at_17, unrated_16, under_17) = (&fx.viewers[1].0, &fx.viewers[2].0, &fx.viewers[3].0);

    get(at_17, alien).await.unwrap();
    let hidden = get(under_17, alien).await.unwrap_err();
    get(under_17, fx.dune).await.unwrap();
    get(unrated_16, alien).await.unwrap_err();

    assert_eq!(hidden.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn request_events_reach_a_capped_user_only_for_titles_their_maximum_age_lets_through() {
    let fx = Fixture::start().await;
    let types = [
        EventType::EVENT_TYPE_REQUEST_CREATED,
        EventType::EVENT_TYPE_REQUEST_STATE_CHANGED,
        EventType::EVENT_TYPE_FOLLOWED_SHOW_CREATED,
        EventType::EVENT_TYPE_REQUESTER_ADDED,
    ];
    let mut subscriptions = Vec::new();
    for (viewer, _) in &fx.viewers {
        subscriptions.push(subscribe(&viewer.client_config(), &types).await);
    }

    fx.hicks_asks().await;
    // Hicks joining the viewers' own Dune comes last and reaches each, so whatever they heard
    // before it is all they hear of the rest.
    fx.server.requests(&fx.hicks).ask(DUNE).await;

    let mut heard = Vec::new();
    for subscription in &mut subscriptions {
        heard.push(until_dune_joined(subscription).await);
    }
    let said = |names: &[&str]| names.iter().map(|&name| name.to_owned()).collect();
    assert_eq!(
        heard,
        [
            said(&[
                "Alien created",
                "Alien declined",
                "Dark created",
                "Dark followed",
                "Heat created"
            ]),
            said(&[
                "Alien created",
                "Alien declined",
                "Dark created",
                "Dark followed"
            ]),
            said(&["Heat created"]),
            BTreeSet::new(),
        ]
    );
}

/// What `subscription` hears until someone joins Dune's Request.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "any other event is heard as itself, which fails the test"
)]
async fn until_dune_joined(subscription: &mut Subscription) -> BTreeSet<String> {
    let mut heard = BTreeSet::new();
    loop {
        let said = match subscription.next().await {
            event::Event::RequesterAdded(added) if title_of(&added.request) == "Dune" => {
                return heard;
            }
            event::Event::RequestCreated(created) => {
                format!("{} created", title_of(&created.request))
            }
            event::Event::RequestStateChanged(changed) => {
                format!("{} declined", title_of(&changed.request))
            }
            event::Event::FollowedShowCreated(created) => {
                format!("{} followed", created.followed_show.show_title)
            }
            other => format!("{other:?}"),
        };
        heard.insert(said);
    }
}
