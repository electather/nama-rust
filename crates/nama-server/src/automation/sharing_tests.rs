use std::sync::Arc;

use nama_plugin_api::{
    automation::{Kernel, KernelRefused, Permission, functions::ListTitlesSharing},
    contract::{SharingTitles, TitleKind, TitlesSharingAsked},
};

use super::super::{
    Automation,
    testing::{ALIEN_TMDB_ID, DARK_TMDB_ID, kernel, two_episodes_in_one_file},
};
use crate::{
    db::Database,
    library::{
        Contents, Metadata,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
};

const ALIENS_TMDB_ID: i64 = 679;

/// A Server whose Movies Library holds Alien and Aliens, both Dramas, and whose Shows Library
/// holds two Episodes of the Drama Dark, with the Kernel's functions granted Read Libraries.
struct Fixture {
    _data: tempfile::TempDir,
    _folders: [tempfile::TempDir; 2],
    _automation: Arc<Automation>,
    kernel: Arc<dyn Kernel>,
}

impl Fixture {
    async fn new() -> Self {
        let data = tempfile::tempdir().unwrap();
        let movies = tempfile::tempdir().unwrap();
        let shows = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let library = movies_library(&db, movies.path()).await;
        let contents = Contents::new(Arc::clone(&db));
        for (tmdb, title) in [(ALIEN_TMDB_ID, "Alien"), (ALIENS_TMDB_ID, "Aliens")] {
            let path = movies
                .path()
                .canonicalize()
                .unwrap()
                .join(format!("{tmdb}.mkv"));
            let drama = Metadata {
                genres: vec!["Drama".to_owned()],
                ..tmdb_movie(tmdb, title)
            };
            MatchedMovie::store(&contents, library, path.to_str().unwrap(), drama).await;
        }
        two_episodes_in_one_file(&db, shows.path()).await;
        let (automation, kernel) = kernel(&db, data.path(), Permission::PERMISSION_READ_LIBRARIES);
        Self {
            _data: data,
            _folders: [movies, shows],
            _automation: automation,
            kernel,
        }
    }

    /// The first id of each title of `kind` `list_titles_sharing` answers for Drama, at most
    /// `limit`, or why it refused.
    async fn dramas(&self, kind: TitleKind, limit: u32) -> Result<Vec<String>, KernelRefused> {
        let asked = TitlesSharingAsked {
            kind: kind.into(),
            genres: vec!["Drama".to_owned()],
            limit,
            ..TitlesSharingAsked::default()
        };
        let answer: SharingTitles = self.kernel.ask::<ListTitlesSharing>(asked).await?;
        Ok((answer.titles.iter())
            .map(|title| title.ids[0].value.clone())
            .collect())
    }
}

#[tokio::test]
async fn list_titles_sharing_gives_a_show_once_not_per_episode_and_only_the_kind_asked() {
    let fx = Fixture::new().await;

    let shows = fx.dramas(TitleKind::TITLE_KIND_SHOW, 0).await.unwrap();
    let movies = fx.dramas(TitleKind::TITLE_KIND_MOVIE, 0).await.unwrap();

    assert_eq!(shows, [DARK_TMDB_ID.to_string()]);
    assert_eq!(
        movies,
        [ALIEN_TMDB_ID.to_string(), ALIENS_TMDB_ID.to_string()]
    );
}

#[tokio::test]
async fn list_titles_sharing_answers_at_most_the_limit_asked() {
    let fx = Fixture::new().await;

    let movies = fx.dramas(TitleKind::TITLE_KIND_MOVIE, 1).await.unwrap();

    assert_eq!(movies, [ALIEN_TMDB_ID.to_string()]);
}

#[tokio::test]
async fn list_titles_sharing_refuses_a_kind_neither_movie_nor_show() {
    let fx = Fixture::new().await;

    let refused = fx.dramas(TitleKind::TITLE_KIND_UNSPECIFIED, 0).await;

    assert_eq!(
        refused.unwrap_err().0,
        "the kind of title must be Movie or Show"
    );
}
