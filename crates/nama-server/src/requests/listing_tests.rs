use std::sync::Arc;

use nama_proto::nama::v1::{PageRequest, Permission, RequestState, request::MovieOrSeason};

use crate::{
    plugins::ExtensionPoints,
    requests::{
        Requests,
        asking::{Wanted, WantedSeasons, ask_movie},
        testing::{
            ADA, BOB, ada, ada_and_bob, ask_for, breaking_bad, dark, follow_as, movie, requests_on,
            user,
        },
    },
    users::{Permissions, User},
};

#[tokio::test]
async fn the_pending_count_is_as_many_as_the_pending_list_shows() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    ask_for(&db, ADA, 949, RequestState::Pending).await;
    ask_for(&db, ADA, 603, RequestState::Approved).await;
    follow_as(&db, ADA, dark(), RequestState::Pending).await;
    follow_as(&db, ADA, breaking_bad(), RequestState::Approved).await;
    let nothing = Permissions::from_iter([]);
    let viewers = [
        user(BOB, Permissions::from_iter([Permission::ViewRequests])),
        ada(nothing),
        user(BOB, nothing),
    ];

    let mut counted_and_listed = Vec::new();
    for viewer in &viewers {
        let counted = requests.count_pending(viewer).await.unwrap();
        let page = requests
            .list(viewer, Some(RequestState::Pending), &sized(100))
            .await
            .unwrap();
        let listed = page.requests.len() + page.followed_shows.len();
        counted_and_listed.push((counted, u32::try_from(listed).unwrap()));
    }

    // Ada's Pending Movie, Followed Show and its two Season Requests.
    assert_eq!(counted_and_listed, [(4, 4), (4, 4), (0, 0)]);
}

fn sized(size: i32) -> PageRequest {
    PageRequest {
        size,
        ..Default::default()
    }
}

#[tokio::test]
async fn no_size_lists_50_and_500_lists_100_and_only_the_first_page_counts_them_all() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    for tmdb_id in 1..=101 {
        ask_for(&db, ADA, tmdb_id, RequestState::Pending).await;
    }
    let viewer = ada(Permissions::from_iter([]));

    let unsized_page = requests.list(&viewer, None, &sized(0)).await.unwrap();
    let first = requests.list(&viewer, None, &sized(500)).await.unwrap();
    let after_first = PageRequest {
        token: first.page.next_token.clone(),
        ..sized(500)
    };
    let second = requests.list(&viewer, None, &after_first).await.unwrap();

    assert_eq!(unsized_page.requests.len(), 50);
    assert_eq!(first.requests.len(), 100);
    assert_eq!(first.page.total_size, Some(101));
    assert_eq!(second.requests.len(), 1);
    assert_eq!(
        second.page.total_size, None,
        "counted on the first page only"
    );
    assert_eq!(second.page.next_token, "", "the last page");
}

#[tokio::test]
async fn a_capped_viewer_pages_and_counts_only_others_requests_their_maximum_age_lets_through() {
    let (_data, db) = ada_and_bob().await;
    let requests = requests_on(Arc::clone(&db), ExtensionPoints::default());
    // Ada's Movies by TMDB id, then Bob's own, over any age.
    let asked = [
        (ADA, 1, Some("R")),
        (ADA, 2, Some("PG-13")),
        (ADA, 3, None),
        (ADA, 4, Some("us:pg")),
        (ADA, 5, Some("Not Rated")),
        (BOB, 6, Some("NC-17")),
    ];
    for (user, tmdb_id, rating) in asked {
        let wanted = Wanted {
            content_rating: rating.map(str::to_owned),
            ..movie(tmdb_id)
        };
        db.write(move |tx| Box::pin(ask_movie(tx, user, wanted, RequestState::Pending)))
            .await
            .unwrap();
    }
    let dark = WantedSeasons {
        content_rating: Some("TV-MA".to_owned()),
        ..dark()
    };
    follow_as(&db, ADA, dark, RequestState::Pending).await;
    let bob = user(BOB, Permissions::from_iter([Permission::ViewRequests]));
    let caps = [(None, false), (Some(13), false), (Some(13), true)];

    let mut seen = Vec::new();
    for (age, allow_unrated) in caps {
        db.write(move |tx| {
            Box::pin(
                sqlx::query("UPDATE users SET max_age = ?, allow_unrated = ? WHERE id = ?")
                    .bind(age)
                    .bind(allow_unrated)
                    .bind(BOB)
                    .execute(&mut **tx),
            )
        })
        .await
        .unwrap();
        seen.push(listed_one_at_a_time(&requests, &bob).await);
    }

    let listed = |names: &[&str]| {
        names
            .iter()
            .map(|&name| name.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        seen,
        [
            (
                listed(&[
                    "Dark 1",
                    "Dark 2",
                    "Movie 1",
                    "Movie 2",
                    "Movie 3",
                    "Movie 4",
                    "Movie 5",
                    "Movie 6",
                    "following Dark",
                ]),
                Some(9),
                9
            ),
            (listed(&["Movie 2", "Movie 4", "Movie 6"]), Some(3), 3),
            (
                listed(&["Movie 2", "Movie 3", "Movie 4", "Movie 5", "Movie 6"]),
                Some(5),
                5
            ),
        ]
    );
}

/// What `viewer` lists, a page of one at a time, sorted by name, with the total the first page
/// gives and how many Pending they count.
async fn listed_one_at_a_time(
    requests: &Requests,
    viewer: &User,
) -> (Vec<String>, Option<i32>, u32) {
    let mut names = Vec::new();
    let mut asked = sized(1);
    let mut total = None;
    loop {
        let page = requests.list(viewer, None, &asked).await.unwrap();
        total = total.or(page.page.total_size);
        names.extend(
            page.requests
                .iter()
                .map(|request| match &request.movie_or_season {
                    Some(MovieOrSeason::Movie(movie)) => movie.title.clone(),
                    Some(MovieOrSeason::Season(season)) => {
                        format!("{} {}", season.show_title, season.season_number)
                    }
                    None => String::new(),
                }),
        );
        names.extend(
            (page.followed_shows.iter())
                .map(|followed| format!("following {}", followed.show_title)),
        );
        if page.page.next_token.is_empty() {
            break;
        }
        asked.token.clone_from(&page.page.next_token);
    }
    names.sort();
    (names, total, requests.count_pending(viewer).await.unwrap())
}
