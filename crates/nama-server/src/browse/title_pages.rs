//! What a title's page shows a User: a Movie or Show in a Library, or a Found one. Every section of
//! a title's page is read here, so a new one is added once.

use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::Permission;

use super::{Browse, Error, Movie, Show};
use crate::{
    db::Database,
    library::{
        MAX_SIMILAR, Metadata, OnServer, Roles, SimilarAnswer, SimilarSources, TitleKind,
        WatchState, merged_similar,
    },
    requests::{self, Found, FoundShow, Requests},
    users::User,
    watchlist,
};

/// A Movie's page, of a Movie in a Library.
#[derive(Debug)]
pub(crate) struct MoviePage {
    pub(crate) movie: Movie,
    /// The User's Watch State of it, if they've started it.
    pub(crate) state: Option<WatchState>,
    pub(crate) roles: Roles,
    pub(crate) on_watchlist: bool,
    pub(crate) similar: Vec<OnServer>,
    /// The Library's folder that refuses deleting it, told only to a User with Delete media:
    /// where a Library lives isn't every User's to see (`crates/nama-server/AGENTS.md`, "Clients
    /// name ids").
    pub(crate) read_only_folder: Option<String>,
}

/// A Show's page, of a Show in a Library.
#[derive(Debug)]
pub(crate) struct ShowPage {
    pub(crate) show: Show,
    /// Its Roles, over its whole run.
    pub(crate) roles: Roles,
    pub(crate) on_watchlist: bool,
    /// How many of its Episodes have aired, Specials aside.
    pub(crate) aired_episodes: u32,
    pub(crate) similar: Vec<OnServer>,
}

/// A Found Movie's or Show's page: `found` already holds its Roles, Trailers, genres and Request
/// state.
#[derive(Debug)]
pub(crate) struct FoundPage<T> {
    pub(crate) found: T,
    pub(crate) on_watchlist: bool,
    pub(crate) similar: Vec<OnServer>,
}

/// Why a Found page couldn't be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum FoundPageError {
    /// Looking the title up failed.
    #[error(transparent)]
    LookUp(#[from] requests::Error),
    #[error(transparent)]
    Browse(#[from] Error),
}

/// Reads what a title's page shows a User.
#[derive(Debug, Clone)]
pub(crate) struct TitlePages {
    browse: Browse,
    requests: Requests,
    similar: SimilarSources,
    db: Arc<Database>,
}

impl TitlePages {
    pub(crate) fn new(db: Arc<Database>, requests: Requests, similar: SimilarSources) -> Self {
        Self {
            browse: Browse::new(Arc::clone(&db)),
            requests,
            similar,
            db,
        }
    }

    /// The page of the Movie `id` of the Movies Library `library`, as `user` sees it.
    pub(crate) async fn movie(
        &self,
        user: &User,
        library: i64,
        id: &ExternalId,
    ) -> Result<MoviePage, Error> {
        let (movie, state, read_only_folder) = self.browse.movie(user.id, library, id).await?;
        Ok(MoviePage {
            on_watchlist: self
                .on_watchlist(user, TitleKind::Movie, &movie.ids)
                .await?,
            similar: self.stored_similar(TitleKind::Movie, &movie.ids).await?,
            roles: self.browse.contents.movie_roles(&movie).await?,
            read_only_folder: read_only_folder
                .filter(|_| user.permissions.allow(Permission::DeleteMedia)),
            movie,
            state,
        })
    }

    /// The page of the Show `id` of the Shows Library `library`, as `user` sees it.
    pub(crate) async fn show(
        &self,
        user: &User,
        library: i64,
        id: &ExternalId,
    ) -> Result<ShowPage, Error> {
        let show = self.browse.show(library, id).await?;
        Ok(ShowPage {
            on_watchlist: self.on_watchlist(user, TitleKind::Show, &show.ids).await?,
            roles: self.browse.contents.show_roles(&show).await?,
            aired_episodes: self.browse.contents.aired_episodes(&show).await?,
            similar: self.stored_similar(TitleKind::Show, &show.ids).await?,
            show,
        })
    }

    /// The page of the Movie `id` the title source has, as `user` sees it.
    pub(crate) async fn found_movie(
        &self,
        user: &User,
        id: &ExternalId,
    ) -> Result<FoundPage<Found<Metadata>>, FoundPageError> {
        let found = self.requests.look_up(user, id).await?;
        let movie = &found.movie;
        let answers = (self.similar)
            .ask(TitleKind::Movie, &movie.ids, &movie.genres, &movie.roles)
            .await;
        Ok(FoundPage {
            on_watchlist: self.found_on_watchlist(user, TitleKind::Movie, id).await?,
            similar: self
                .similar_in_libraries(TitleKind::Movie, &answers)
                .await?,
            found,
        })
    }

    /// The page of the Show `id` the title source has, as `user` sees it.
    pub(crate) async fn found_show(
        &self,
        user: &User,
        id: &ExternalId,
    ) -> Result<FoundPage<FoundShow>, FoundPageError> {
        let found = self.requests.look_up_show(user, id).await?;
        let show = &found.show;
        let answers = (self.similar)
            .ask(TitleKind::Show, &show.ids, &show.genres, &show.roles)
            .await;
        Ok(FoundPage {
            on_watchlist: self.found_on_watchlist(user, TitleKind::Show, id).await?,
            similar: self.similar_in_libraries(TitleKind::Show, &answers).await?,
            found,
        })
    }

    /// Whether a title in a Library, of `kind` and known by `ids`, is on `user`'s Watchlist under
    /// any of them.
    async fn on_watchlist(
        &self,
        user: &User,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Result<bool, Error> {
        Ok(watchlist::has(&self.db, user.id, kind, ids).await?)
    }

    /// Whether a Found title, of `kind` and looked up by `id`, is on `user`'s Watchlist. By that
    /// id, which its page's Watchlist button removes it under, not the title source's other ids:
    /// `RemoveFromWatchlist` clears only the Matches' (#1292).
    async fn found_on_watchlist(
        &self,
        user: &User,
        kind: TitleKind,
        id: &ExternalId,
    ) -> Result<bool, Error> {
        Ok(watchlist::has_matched(&self.db, user.id, kind, id).await?)
    }

    /// The Similar titles of the title in a Library of `kind` known by `ids`, from what each
    /// enabled Similar source last answered about it (ADR 0081).
    async fn stored_similar(
        &self,
        kind: TitleKind,
        ids: &ExternalIds,
    ) -> Result<Vec<OnServer>, Error> {
        let stored = self.browse.contents.similar(kind, ids).await?;
        let answers = self.similar.of_enabled(stored);
        self.similar_in_libraries(kind, &answers).await
    }

    /// The first [`MAX_SIMILAR`] titles of `answers`, merged (ADR 0084), that a Library has, as
    /// [`Browse::in_libraries`] finds them: a title two sources answer, by any id they share,
    /// once, at its heaviest weight.
    async fn similar_in_libraries(
        &self,
        kind: TitleKind,
        answers: &[SimilarAnswer],
    ) -> Result<Vec<OnServer>, Error> {
        let mut similar = (self.browse)
            .in_libraries(kind, &merged_similar(answers))
            .await?;
        similar.truncate(MAX_SIMILAR);
        Ok(similar)
    }
}

#[cfg(test)]
#[path = "title_pages_tests.rs"]
mod tests;
