use std::{collections::BTreeSet, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::Whereabouts;
use crate::{
    content_ratings::MaximumAge,
    db::Database,
    library::{
        Contents, Libraries, LibraryKind, Metadata, OnServer, TitleKind,
        fixture::{MatchedMovie, tmdb_movie},
    },
    plugins::ExtensionPoints,
    users::{LibraryAccess, Permissions, Reach, User, Users, reach, testing::insert_user},
};

/// The Library and id each of `found` is found by.
fn found_as(found: &[OnServer]) -> Vec<(i64, ExternalId)> {
    (found.iter())
        .map(|title| match title {
            OnServer::Movie(movie) => (movie.library, movie.id.clone()),
            OnServer::Show(show) => (show.library, show.id.clone()),
        })
        .collect()
}

#[tokio::test]
async fn titles_are_found_in_the_order_named_once_each_in_the_oldest_library() {
    let (data, older, newer) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = MatchedMovie::new(&db, older.path(), tmdb_movie(348, "Alien")).await;
    let heat = MatchedMovie::new(&db, newer.path(), tmdb_movie(949, "Heat")).await;
    let contents = Contents::new(Arc::clone(&db));
    let alien_again = tmdb_movie(348, "Alien");
    MatchedMovie::store(&contents, heat.library, "/media/Alien.mkv", alien_again).await;
    let libraries = Libraries::new(db).list().await.unwrap();
    let named = [949, 1, 348, 949].map(ExternalId::tmdb);
    let everything = Reach::everything();

    let found = Whereabouts::reached_in_order(
        &contents,
        &libraries,
        &everything,
        LibraryKind::Movies,
        &named,
    )
    .await;

    assert_eq!(
        found_as(&found.unwrap()),
        [
            (heat.library, ExternalId::tmdb(949)),
            (alien.library, ExternalId::tmdb(348)),
        ]
    );
}

#[tokio::test]
async fn a_title_matched_under_another_namespace_is_found_by_any_id_it_shares() {
    let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let tvdb = ExternalId::tvdb(1_234);
    // No TMDB id, so the Library lists it under another of its ids than the one asked for.
    let by_another_source = Metadata {
        plugin: PluginId::from_static("another"),
        ids: ExternalIds::from_iter([imdb.clone(), tvdb.clone()]),
        ..tmdb_movie(348, "Alien")
    };
    let alien = MatchedMovie::new(&db, media.path(), by_another_source).await;
    let libraries = Libraries::new(Arc::clone(&db)).list().await.unwrap();
    let contents = Contents::new(db);
    let everything = Reach::everything();

    let found = Whereabouts::reached_in_order(
        &contents,
        &libraries,
        &everything,
        LibraryKind::Movies,
        &[tvdb],
    )
    .await;

    assert_eq!(found_as(&found.unwrap()), [(alien.library, imdb)]);
}

/// Two Movies Libraries sharing Alien: the older rated R, the newer PG.
struct Shared {
    db: Arc<Database>,
    users: Users,
    older: i64,
    newer: i64,
    _folders: [tempfile::TempDir; 3],
}

impl Shared {
    async fn new() -> Self {
        let folders = [(); 3].map(|()| tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(folders[0].path()).await.unwrap());
        let rated = |rating: &str| Metadata {
            content_rating: Some(rating.to_owned()),
            ..tmdb_movie(348, "Alien")
        };
        let older = MatchedMovie::new(&db, folders[1].path(), rated("R")).await;
        let newer = MatchedMovie::new(&db, folders[2].path(), rated("PG")).await;
        Self {
            users: Users::new(Arc::clone(&db), ExtensionPoints::default()),
            db,
            older: older.library,
            newer: newer.library,
            _folders: folders,
        }
    }

    /// The Reach of a new User who sees `libraries`, `None` for every one, under `max_age`.
    async fn reach(&self, libraries: Option<&[i64]>, max_age: Option<u32>) -> Reach {
        let user = insert_user(&self.db, &format!("User {libraries:?} {max_age:?}")).await;
        let admin = User {
            id: 0,
            name: "Admin".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        };
        let access = libraries.map_or(LibraryAccess::All, |libraries| {
            LibraryAccess::Only(BTreeSet::from_iter(libraries.iter().copied()))
        });
        let users = &self.users;
        users
            .set_library_access(&admin, user, access)
            .await
            .unwrap();
        let max_age = MaximumAge {
            age: max_age,
            allow_unrated: true,
        };
        users.set_maximum_age(&admin, user, max_age).await.unwrap();
        reach(&self.db, user).await.unwrap()
    }

    /// Where Alien is for `reach`, by the Library it is in, as a page of titles finds it and as
    /// asking for a Request does, which must agree.
    async fn alien_for(&self, reach: &Reach) -> Whereabouts<i64> {
        let contents = Contents::new(Arc::clone(&self.db));
        let libraries = Libraries::new(Arc::clone(&self.db)).list().await.unwrap();
        let alien = ExternalId::tmdb(348);
        let of_titles = Whereabouts::of_titles(
            &contents,
            &libraries,
            reach,
            LibraryKind::Movies,
            std::slice::from_ref(&alien),
        );
        let of_titles = match of_titles.await.unwrap().remove(&alien) {
            Some(Whereabouts::In(title)) => Whereabouts::In(title.library()),
            Some(Whereabouts::KeptFrom) => Whereabouts::KeptFrom,
            Some(Whereabouts::Nowhere) | None => Whereabouts::Nowhere,
        };
        let mut conn = self.db.reader().acquire().await.unwrap();
        let of_title = Whereabouts::of_title(&mut conn, reach, TitleKind::Movie, &alien);
        let of_title = of_title.await.unwrap();
        assert_eq!(of_titles, of_title, "the two readers disagree");
        of_title
    }
}

#[tokio::test]
async fn a_shared_title_is_in_the_oldest_library_a_user_sees_that_holds_it() {
    let shared = Shared::new().await;
    let (older, newer) = (shared.older, shared.newer);
    let table = [
        (None, Whereabouts::In(older)),
        (Some(&[older, newer][..]), Whereabouts::In(older)),
        (Some(&[newer][..]), Whereabouts::In(newer)),
        (Some(&[][..]), Whereabouts::KeptFrom),
    ];

    for (libraries, expected) in table {
        let reach = shared.reach(libraries, None).await;
        assert_eq!(shared.alien_for(&reach).await, expected, "{libraries:?}");
    }
}

#[tokio::test]
async fn the_maximum_age_judges_the_copy_in_the_oldest_library_a_user_sees() {
    let shared = Shared::new().await;
    let (older, newer) = (shared.older, shared.newer);

    let every_library = shared.reach(None, Some(13)).await;
    let only_newer = shared.reach(Some(&[newer]), Some(13)).await;
    let only_older = shared.reach(Some(&[older]), Some(13)).await;

    assert_eq!(
        shared.alien_for(&every_library).await,
        Whereabouts::KeptFrom
    );
    assert_eq!(shared.alien_for(&only_newer).await, Whereabouts::In(newer));
    assert_eq!(shared.alien_for(&only_older).await, Whereabouts::KeptFrom);
}

#[tokio::test]
async fn a_title_no_library_holds_is_nowhere() {
    let shared = Shared::new().await;
    let reach = shared.reach(None, None).await;
    let mut conn = shared.db.reader().acquire().await.unwrap();

    let heat = ExternalId::tmdb(949);
    let found = Whereabouts::of_title(&mut conn, &reach, TitleKind::Movie, &heat).await;

    assert_eq!(found.unwrap(), Whereabouts::Nowhere);
}
