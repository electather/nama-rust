//! Box Sets as a User sees them (`GLOSSARY.md`): on a Movie's page, on their own page and in a
//! Movies Library's list. Every read applies [`TitlePages::seen`], so the rule of who sees a Box
//! Set and which of its Movies lives once.

use std::collections::HashMap;

use nama_plugin_api::{PluginId, metadata::ExternalId};
use nama_proto::nama::v1::{PageRequest, PageResponse, Permission};

use super::{Error, Movie, TitlePages};
use crate::{
    home_rows::RowTitle,
    library::{
        FoundMovie, ImageRef, LibraryKind, ListedMovie, NotFound, OnServer, StoredBoxSet, TitleKind,
    },
    page::Page,
    requests::Listed,
    users::User,
};

/// A Box Set as a User sees it.
#[derive(Debug)]
pub(crate) struct SeenBoxSet {
    pub(crate) box_set: StoredBoxSet,
    /// Its Movies on the Server, each in the oldest Library that has it, and those it lacks if
    /// the User may Request Movies, in release order.
    pub(crate) movies: Vec<RowTitle>,
}

impl TitlePages {
    /// The Box Set of `movie` as `user` sees it, with each missing Movie's open Request they may
    /// see, if it has one they can see.
    pub(super) async fn box_set_of(
        &self,
        user: &User,
        movie: &Movie,
    ) -> Result<Option<SeenBoxSet>, Error> {
        let Some(id) = self.browse.contents.movie_box_set(movie).await? else {
            return Ok(None);
        };
        match self.box_set(user, &id).await {
            Ok(seen) => Ok(Some(seen)),
            Err(Error::NotFound(NotFound::NoSuchBoxSet)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// The Box Set `id` as `user` sees it, with each missing Movie's open Request they may see.
    /// [`NotFound::NoSuchBoxSet`] if they can't see it.
    pub(crate) async fn box_set(&self, user: &User, id: &ExternalId) -> Result<SeenBoxSet, Error> {
        let seen_by = self.seen_by(user).await?;
        let stored = self.browse.contents.box_set(id).await?;
        let seen = match stored {
            Some(stored) => seen_by.seen(user, stored).await?,
            None => None,
        };
        let SeenBoxSet { box_set, movies } = seen.ok_or(NotFound::NoSuchBoxSet)?;
        let mut with_requests = Vec::with_capacity(movies.len());
        // ponytail: one query per missing Movie, as a Home row's titles read them.
        for title in movies {
            with_requests.push(match title {
                RowTitle::MissingMovie(movie, _) => {
                    let request = self.requests.open_movie_request(user, &movie.id).await?;
                    RowTitle::MissingMovie(movie, request.map(Box::new))
                }
                title @ (RowTitle::OnServer(_) | RowTitle::MissingShow(_)) => title,
            });
        }
        Ok(SeenBoxSet {
            box_set,
            movies: with_requests,
        })
    }

    /// The `asked` page of the Box Sets with a Movie in the Movies Library `library` that `user`
    /// sees, by name.
    pub(crate) async fn box_sets(
        &self,
        user: &User,
        library: i64,
        asked: &PageRequest,
    ) -> Result<(Vec<StoredBoxSet>, PageResponse), Error> {
        let this = self.seen_by(user).await?;
        let library = this.browse.library(library).await?;
        if library.kind != LibraryKind::Movies {
            return Err(NotFound::WrongKind {
                wanted: LibraryKind::Movies,
            }
            .into());
        }
        let list = format!("Box Sets of Library {}", library.id);
        let page = Page::<()>::read(asked, list.as_bytes())?;
        let mut seen = Vec::new();
        // ponytail: reads every Box Set of the Library, and its Movies in every Library, to page
        // in memory by place, so one added between two pages can skip or repeat one. Store which
        // Box Sets each User sees if a Library's grow to hundreds.
        for id in self.browse.contents.box_sets_in(library.id).await? {
            let Some(stored) = self.browse.contents.box_set(&id).await? else {
                continue;
            };
            if let Some(shown) = this.seen(user, stored).await? {
                seen.push(shown.box_set);
            }
        }
        seen.sort_by_cached_key(|box_set| (box_set.name.to_lowercase(), box_set.id.to_string()));
        let total = seen.len();
        let mut listed: Vec<(i64, StoredBoxSet)> = (0..)
            .zip(seen)
            .filter(|&(place, _)| page.after.is_none_or(|((), after)| place > after))
            .collect();
        let mut paging = page.finish(&mut listed, |&(place, _)| ((), place));
        if page.is_first() {
            paging.total_size = Some(i32::try_from(total).unwrap_or(i32::MAX));
        }
        Ok((
            listed.into_iter().map(|(_, box_set)| box_set).collect(),
            paging,
        ))
    }

    /// `stored` as `user` sees it, `None` if they can't: they see a Box Set only when a Library
    /// has one of the Movies naming it and they see at least two, counting those it lacks only if
    /// they may Request Movies. Its Movies come in its title source's order, with the others
    /// naming it placed among them by release date.
    async fn seen(&self, user: &User, stored: StoredBoxSet) -> Result<Option<SeenBoxSet>, Error> {
        let mut ids = stored.listed.clone();
        ids.extend(
            (stored.named_by.iter())
                .filter(|id| !stored.listed.contains(id))
                .cloned(),
        );
        let mut on_server: Vec<Option<FoundMovie>> = (self.browse)
            .in_libraries(TitleKind::Movie, &ids)
            .await?
            .into_iter()
            .filter_map(|title| match title {
                OnServer::Movie(movie) => Some(Some(movie)),
                OnServer::Show(_) => None,
            })
            .collect();
        let may_request = user.permissions.allow(Permission::RequestMovie);
        let looked_up: HashMap<&str, &ListedMovie> = (stored.looked_up.iter())
            .map(|listed| (listed.listed_as.as_str(), listed))
            .collect();
        let mut movies = Vec::new();
        for id in &stored.listed {
            let found = (on_server.iter_mut())
                .find(|movie| movie.as_ref().is_some_and(|movie| has_id(movie, id)));
            match found {
                Some(found) => movies.extend(found.take().map(on_server_title)),
                None if may_request => movies.extend(
                    (looked_up.get(id.to_string().as_str()))
                        .and_then(|listed| missing_title(listed, &stored.plugin)),
                ),
                None => {}
            }
        }
        let mut named_only: Vec<FoundMovie> = on_server.into_iter().flatten().collect();
        named_only
            .sort_by_cached_key(|movie| (movie.release_date.is_none(), movie.release_date.clone()));
        for movie in named_only {
            let at = (movie.release_date.as_deref())
                .and_then(|date| {
                    (movies.iter()).position(|title| released(title).is_none_or(|it| *it > *date))
                })
                .unwrap_or(movies.len());
            movies.insert(at, on_server_title(movie));
        }
        // A Box Set's row outlives its Movies' Matches, so one whose Movies all moved to another
        // Box Set, or left the Server, would still list those its stale listing names.
        let named_on_server = (movies.iter()).any(|title| match title {
            RowTitle::OnServer(OnServer::Movie(movie)) => {
                (stored.named_by.iter()).any(|id| has_id(movie, id))
            }
            RowTitle::OnServer(OnServer::Show(_))
            | RowTitle::MissingMovie(..)
            | RowTitle::MissingShow(_) => false,
        });
        Ok(
            (named_on_server && movies.len() >= 2).then_some(SeenBoxSet {
                box_set: stored,
                movies,
            }),
        )
    }
}

/// Whether `movie` is known by `id`.
fn has_id(movie: &FoundMovie, id: &ExternalId) -> bool {
    movie.id == *id || movie.ids.iter().any(|known| known == *id)
}

/// When `title` came out, as text that sorts by date: a missing Movie's year alone, which sorts
/// before the dates of that year.
fn released(title: &RowTitle) -> Option<std::borrow::Cow<'_, str>> {
    match title {
        RowTitle::OnServer(OnServer::Movie(movie)) => movie.release_date.as_deref().map(Into::into),
        RowTitle::MissingMovie(movie, _) => movie.year.map(|year| year.to_string().into()),
        RowTitle::OnServer(OnServer::Show(_)) | RowTitle::MissingShow(_) => None,
    }
}

fn on_server_title(movie: FoundMovie) -> RowTitle {
    RowTitle::OnServer(OnServer::Movie(movie))
}

/// `listed`, which the Server lacks, as a Box Set shows it, its poster from `plugin`'s title
/// source, unless its id no longer reads.
fn missing_title(listed: &ListedMovie, plugin: &PluginId) -> Option<RowTitle> {
    let listed = Listed {
        id: listed.id.parse().ok()?,
        title: listed.title.clone(),
        year: listed.year,
        poster: ImageRef::of(plugin, listed.poster.as_ref()),
    };
    Some(RowTitle::MissingMovie(listed, None))
}
