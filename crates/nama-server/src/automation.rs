//! Automation: the Kernel's functions an Automation Plugin calls (ADR 0048), each checking the
//! Plugin Permission it needs before anything else.

mod store;

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
};

use buffa::{EnumValue, MessageField};
use nama_plugin_api::{
    BoxFuture, PluginId,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, KernelRefused, Libraries, LibrariesAsked, Library,
        LibraryKind, Permission, Requesters, RequestersAsked, StoreEntry, StoreKey, StoreValue,
        Stored, Title, TitleRef, TitleRefKind, TitleRequesters, TitleWatchStates, Titles,
        TitlesAsked, User, Users, UsersAsked, WatchState, WatchStates, WatchStatesAsked,
    },
    fulfiller::TitleId,
    metadata::{ExternalId, ExternalIds},
};
use tracing::error;

use crate::{
    browse::{self, Browse, Target, ToDelete},
    db::Database,
    library::{self, Contents, ItemKind, LibraryEpisode, LibraryError, WatchStateKind},
    plugins::ExtensionPoints,
    requests,
    scanner::{self, Scanner},
    users,
};

/// The most titles one page of `list_titles` holds.
const MAX_PAGE_SIZE: usize = 1000;

/// What makes the Kernel's functions for each enabled Automation Plugin.
///
/// The Plugins it serves hold it only weakly, since it holds them in turn through the Scanner and
/// the Extension Points: whoever starts the Server holds it, until the Plugins have stopped.
pub(crate) struct Automation {
    db: Arc<Database>,
    browse: Browse,
    scanner: Scanner,
    points: ExtensionPoints,
}

impl Automation {
    /// Serves the Kernel's functions to each Automation Plugin `points` enables.
    pub(crate) fn serve(db: Arc<Database>, scanner: Scanner, points: ExtensionPoints) -> Arc<Self> {
        let automation = Arc::new(Self {
            browse: Browse::new(Arc::clone(&db)),
            db,
            scanner,
            points,
        });
        let kernels = Arc::downgrade(&automation);
        automation
            .points
            .serve_kernel(Arc::new(move |plugin, granted| {
                Self::kernel(&kernels, plugin, granted)
            }));
        automation
    }

    /// The Kernel's functions as `plugin` calls them, granted `granted`. Each refuses once
    /// `automation` is gone, as the Server stops.
    fn kernel(
        automation: &Weak<Self>,
        plugin: PluginId,
        granted: HashSet<Permission>,
    ) -> Arc<dyn Kernel> {
        Arc::new(PluginKernel {
            automation: Weak::clone(automation),
            plugin,
            granted,
        })
    }
}

/// A function of the Kernel's, for the one table of what each needs.
#[derive(Debug, Clone, Copy)]
enum Method {
    StoreGet,
    StorePut,
    StoreDelete,
    ListLibraries,
    ListTitles,
    ListWatchStates,
    ListUsers,
    ListRequesters,
    DeleteMedia,
}

impl Method {
    /// The one place that decides which Permission each function needs.
    const fn needs(self) -> Permission {
        match self {
            Self::StoreGet | Self::StorePut | Self::StoreDelete => Permission::PERMISSION_STORE,
            Self::ListLibraries | Self::ListTitles => Permission::PERMISSION_READ_LIBRARIES,
            Self::ListWatchStates => Permission::PERMISSION_READ_WATCH_STATES,
            Self::ListUsers => Permission::PERMISSION_READ_USERS,
            Self::ListRequesters => Permission::PERMISSION_READ_REQUESTS,
            Self::DeleteMedia => Permission::PERMISSION_DELETE_MEDIA,
        }
    }
}

/// How an Admin reads a Permission's name.
const fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::PERMISSION_UNSPECIFIED => "unnamed",
        Permission::PERMISSION_STORE => "Store",
        Permission::PERMISSION_READ_LIBRARIES => "Read Libraries",
        Permission::PERMISSION_READ_WATCH_STATES => "Read Watch States",
        Permission::PERMISSION_DELETE_MEDIA => "Delete media",
        Permission::PERMISSION_READ_USERS => "Read Users",
        Permission::PERMISSION_READ_REQUESTS => "Read Requests",
    }
}

struct PluginKernel {
    /// Held weakly: see [`Automation`].
    automation: Weak<Automation>,
    plugin: PluginId,
    granted: HashSet<Permission>,
}

impl PluginKernel {
    /// Checks the Plugin was granted what `method` needs, then hands out the Automation.
    fn check(&self, method: Method) -> Result<Arc<Automation>, KernelRefused> {
        let needs = method.needs();
        if !self.granted.contains(&needs) {
            return Err(KernelRefused(format!(
                "the Admin hasn't granted {} the {} Permission",
                self.plugin,
                permission_name(needs)
            )));
        }
        (self.automation.upgrade())
            .ok_or_else(|| KernelRefused("the Server is stopping".to_owned()))
    }

    /// Refuses with the Store's reason, or, for a database failure, logs it and refuses without it.
    fn store_refused(&self, err: &store::Error) -> KernelRefused {
        if let store::Error::Database(err) = err {
            error!(plugin = %self.plugin, "the Store failed: {err}");
            return KernelRefused("the Kernel couldn't reach the Store".to_owned());
        }
        KernelRefused(err.to_string())
    }

    /// Logs a database failure while `doing` something, and refuses without its cause.
    fn database_refused(&self, doing: &str, err: &impl std::fmt::Display) -> KernelRefused {
        error!(plugin = %self.plugin, "{doing} failed: {err}");
        KernelRefused(format!("the Kernel failed {doing}"))
    }

    /// The page of the Library's titles `asked` names: the first `page_size` after the title its
    /// page token names, or every title for a `page_size` of 0. One query, whatever the
    /// Library's size.
    async fn titles_page(
        &self,
        automation: &Automation,
        asked: &TitlesAsked,
    ) -> Result<Titles, KernelRefused> {
        // The token is the id of the last title of the page before, so a title Matched since
        // neither repeats nor skips one (ADR 0048).
        let after = match asked.page_token.as_str() {
            "" => None,
            token => Some(token.parse::<ExternalId>().map_err(|_malformed| {
                KernelRefused(format!(
                    "the page token {token:?} isn't one the Kernel handed out"
                ))
            })?),
        };
        let library = asked.library_id;
        let found = library::Libraries::new(Arc::clone(&automation.db))
            .get(library)
            .await;
        let (kind, title_kind) = match found {
            Ok(found) => match found.kind {
                library::LibraryKind::Movies => {
                    (ItemKind::Movie, TitleRefKind::TITLE_REF_KIND_MOVIE)
                }
                library::LibraryKind::Shows => {
                    (ItemKind::Episode, TitleRefKind::TITLE_REF_KIND_EPISODE)
                }
            },
            Err(LibraryError::Database(err)) => {
                return Err(self.database_refused("reading the Library", &err));
            }
            Err(err) => return Err(KernelRefused(err.to_string())),
        };
        let (after, size) = match asked.page_size {
            0 => (None, None),
            size => {
                let size =
                    usize::try_from(size).map_or(MAX_PAGE_SIZE, |size| size.min(MAX_PAGE_SIZE));
                (after, Some(size))
            }
        };
        // One more than the page holds, to tell whether a page follows.
        let mut listed = Contents::new(Arc::clone(&automation.db))
            .titles_page(library, kind, after.as_ref(), size.map(|size| size + 1))
            .await
            .map_err(|err| self.database_refused("reading the Library's titles", &err))?;
        let next_page_token = match size {
            Some(size) if listed.len() > size => {
                listed.truncate(size);
                listed.last().map_or_default(|last| last.id.to_string())
            }
            _ => String::new(),
        };
        let titles = (listed.into_iter())
            .map(|listed| Title {
                title_ref: MessageField::some(title_ref(library, title_kind, &listed.id)),
                title: listed.title,
                show_title: listed.show_title,
                season_number: listed.season.and_then(|season| u32::try_from(season).ok()),
                episode_number: (listed.episode).and_then(|number| u32::try_from(number).ok()),
                ..Title::default()
            })
            .collect();
        Ok(Titles {
            titles,
            next_page_token,
            ..Titles::default()
        })
    }

    /// Each of `titles` as a Watch State kind and every id its Library's Match gives it, any of
    /// which its Watch State may be kept under: the preferred id follows each Library's title
    /// source order (ADR 0042). A title its Library doesn't have keeps only the id it is named by.
    async fn every_id(
        &self,
        automation: &Automation,
        titles: &[TitleRef],
    ) -> Result<Vec<(WatchStateKind, ExternalIds)>, KernelRefused> {
        let contents = Contents::new(Arc::clone(&automation.db));
        let read = |err| self.database_refused("reading the Library's titles", &err);
        // One read per Library and kind of the titles asked about, each answer going back to the
        // place its title was asked in.
        let mut asked: HashMap<(i64, WatchStateKind), Vec<(usize, ExternalId)>> = HashMap::new();
        for (place, title) in titles.iter().enumerate() {
            let (kind, named) = titled(
                title,
                |id| (WatchStateKind::Movie, id),
                |id| (WatchStateKind::Episode, id),
            )?;
            (asked.entry((title.library_id, kind)).or_default()).push((place, named));
        }
        let mut every_id = Vec::with_capacity(titles.len());
        for ((library, kind), named) in asked {
            let (places, named): (Vec<usize>, Vec<ExternalId>) = named.into_iter().unzip();
            let item_kind = match kind {
                WatchStateKind::Movie => ItemKind::Movie,
                WatchStateKind::Episode => ItemKind::Episode,
            };
            let found = (contents.every_id(library, item_kind, &named).await).map_err(read)?;
            every_id
                .extend((places.into_iter().zip(found)).map(|(place, ids)| (place, (kind, ids))));
        }
        every_id.sort_unstable_by_key(|&(place, _)| place);
        Ok(every_id.into_iter().map(|(_, title)| title).collect())
    }

    /// The ids of the Users who Requested `title`, an Episode's being its Season's, or `None` if
    /// nothing did. A Movie's Requests are found by every id its Match gives it, as its Watch
    /// State is; an Episode the Library doesn't have has none.
    async fn requesters(
        &self,
        automation: &Automation,
        title: &TitleRef,
    ) -> Result<Option<Vec<i64>>, KernelRefused> {
        let contents = Contents::new(Arc::clone(&automation.db));
        let read = |err| self.database_refused("reading the Library's titles", &err);
        let library = title.library_id;
        let (ids, season) = match titled(
            title,
            |id| (ItemKind::Movie, id),
            |id| (ItemKind::Episode, id),
        )? {
            (ItemKind::Movie, id) => {
                let every_id = contents.every_id(library, ItemKind::Movie, &[id]).await;
                (every_id.map_err(read)?.pop().unwrap_or_default(), None)
            }
            (ItemKind::Episode, id) => {
                match contents.season_of(library, &id).await.map_err(read)? {
                    Some((show, season)) => (show, Some(season)),
                    None => return Ok(None),
                }
            }
        };
        let requests = |err| self.database_refused("reading the Requests", &err);
        let mut conn = automation.db.reader().acquire().await.map_err(requests)?;
        (requests::who_requested(&mut conn, &ids, season).await).map_err(requests)
    }

    /// Refuses a delete that would take along one of the Episodes `taken_along`, of the Library
    /// `library`, that `also` doesn't name, naming each one.
    async fn check_taken_along(
        &self,
        automation: &Automation,
        library: i64,
        taken_along: &[LibraryEpisode],
        also: &[TitleRef],
    ) -> Result<(), KernelRefused> {
        let unnamed: Vec<&LibraryEpisode> = (taken_along.iter())
            .filter(|episode| {
                let taken = title_ref(library, TitleRefKind::TITLE_REF_KIND_EPISODE, &episode.id);
                !also.iter().any(|named| same_title(named, &taken))
            })
            .collect();
        if unnamed.is_empty() {
            return Ok(());
        }
        // Only for the refusal, so the Plugin can tell which Episodes they are.
        let ids: Vec<ExternalId> = unnamed.iter().map(|episode| episode.id.clone()).collect();
        let shows = (Contents::new(Arc::clone(&automation.db)))
            .show_titles(library, &ids)
            .await
            .map_err(|err| self.database_refused("reading the Episodes' Shows", &err))?;
        let named: Vec<String> = (unnamed.into_iter().zip(shows))
            .map(|(episode, show)| episode_name(show.as_deref().unwrap_or_default(), episode))
            .collect();
        Err(KernelRefused(format!(
            "the title's files also hold {}; name each in `also` to delete it too, or keep the \
             files",
            named.join(", ")
        )))
    }
}

/// Whether `a` and `b` name the same title of the same Library.
fn same_title(a: &TitleRef, b: &TitleRef) -> bool {
    let id = |title: &TitleRef| {
        (title.id.as_option()).map(|id| (id.namespace.clone(), id.value.clone()))
    };
    a.library_id == b.library_id && a.kind == b.kind && id(a) == id(b)
}

/// An Episode of the Show `show` as a refusal names it, such as `Dark S01E02 (Lies)`.
fn episode_name(show: &str, episode: &LibraryEpisode) -> String {
    format!(
        "{show} S{:02}E{:02} ({})",
        episode.season, episode.number, episode.title
    )
}

/// `id` of a Movie or Episode of the Library `library`, as a Plugin names it.
fn title_ref(library: i64, kind: TitleRefKind, id: &ExternalId) -> TitleRef {
    TitleRef {
        library_id: library,
        kind: kind.into(),
        id: MessageField::some(TitleId {
            namespace: id.namespace().to_owned(),
            value: id.value().to_owned(),
            ..TitleId::default()
        }),
        ..TitleRef::default()
    }
}

/// The Movie or Episode `title` names, as `movie(id)` or `episode(id)`, or why it names none.
fn titled<T>(
    title: &TitleRef,
    movie: impl FnOnce(ExternalId) -> T,
    episode: impl FnOnce(ExternalId) -> T,
) -> Result<T, KernelRefused> {
    let id = title
        .id
        .as_option()
        .ok_or_else(|| KernelRefused("the title's id is missing".to_owned()))?;
    let id = ExternalId::new(&id.namespace, id.value.clone())
        .map_err(|err| KernelRefused(format!("the title's id is malformed: {err}")))?;
    match title.kind {
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_MOVIE) => Ok(movie(id)),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_EPISODE) => Ok(episode(id)),
        EnumValue::Known(TitleRefKind::TITLE_REF_KIND_UNSPECIFIED) | EnumValue::Unknown(_) => Err(
            KernelRefused("the kind of title must be Movie or Episode".to_owned()),
        ),
    }
}

/// What `state`, kept for the User `user`, is to a Plugin.
fn watch_state_to_proto(user: i64, state: library::WatchState) -> WatchState {
    WatchState {
        user_id: user,
        played: state.played,
        position_ms: u64::try_from(state.position_ms).unwrap_or_default(),
        duration_ms: u64::try_from(state.duration_ms).unwrap_or_default(),
        updated_at_ms: state.updated_at.saturating_mul(1000),
        ..WatchState::default()
    }
}

impl Kernel for PluginKernel {
    fn store_get<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<StoreValue, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::StoreGet)?;
            let value = store::get(&automation.db, &self.plugin, key.user_id, &key.key)
                .await
                .map_err(|err| self.store_refused(&err))?;
            Ok(StoreValue {
                value,
                ..StoreValue::default()
            })
        })
    }

    fn store_put<'a>(
        &'a self,
        entry: &'a StoreEntry,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::StorePut)?;
            store::put(
                &automation.db,
                &self.plugin,
                entry.user_id,
                entry.key.clone(),
                entry.value.clone(),
            )
            .await
            .map_err(|err| self.store_refused(&err))?;
            Ok(Stored::default())
        })
    }

    fn store_delete<'a>(
        &'a self,
        key: &'a StoreKey,
    ) -> BoxFuture<'a, Result<Stored, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::StoreDelete)?;
            store::delete(&automation.db, &self.plugin, key.user_id, key.key.clone())
                .await
                .map_err(|err| self.store_refused(&err))?;
            Ok(Stored::default())
        })
    }

    fn list_libraries<'a>(
        &'a self,
        _asked: &'a LibrariesAsked,
    ) -> BoxFuture<'a, Result<Libraries, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListLibraries)?;
            let listed = library::Libraries::new(Arc::clone(&automation.db))
                .list()
                .await
                .map_err(|err| {
                    error!(plugin = %self.plugin, "listing the Libraries failed: {err}");
                    KernelRefused("the Kernel couldn't read the Libraries".to_owned())
                })?;
            Ok(Libraries {
                libraries: listed
                    .into_iter()
                    .map(|library| Library {
                        id: library.id,
                        name: library.name,
                        kind: match library.kind {
                            library::LibraryKind::Movies => LibraryKind::LIBRARY_KIND_MOVIES,
                            library::LibraryKind::Shows => LibraryKind::LIBRARY_KIND_SHOWS,
                        }
                        .into(),
                        ..Library::default()
                    })
                    .collect(),
                ..Libraries::default()
            })
        })
    }

    fn list_titles<'a>(
        &'a self,
        asked: &'a TitlesAsked,
    ) -> BoxFuture<'a, Result<Titles, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListTitles)?;
            self.titles_page(&automation, asked).await
        })
    }

    fn list_watch_states<'a>(
        &'a self,
        asked: &'a WatchStatesAsked,
    ) -> BoxFuture<'a, Result<WatchStates, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListWatchStates)?;
            let titles = self.every_id(&automation, &asked.titles).await?;
            let states = (library::WatchStates::new(Arc::clone(&automation.db)))
                .of_titles(None, &titles)
                .await
                .map_err(|err| self.database_refused("reading the Watch States", &err))?;
            Ok(WatchStates {
                titles: (asked.titles.iter().zip(states))
                    .map(|(title, states)| TitleWatchStates {
                        title_ref: MessageField::some(title.clone()),
                        states: (states.into_iter())
                            .map(|(user, state)| watch_state_to_proto(user, state))
                            .collect(),
                        ..TitleWatchStates::default()
                    })
                    .collect(),
                ..WatchStates::default()
            })
        })
    }

    fn list_users<'a>(
        &'a self,
        _asked: &'a UsersAsked,
    ) -> BoxFuture<'a, Result<Users, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListUsers)?;
            let listed = users::Users::new(Arc::clone(&automation.db), automation.points.clone())
                .list()
                .await
                .map_err(|err| self.database_refused("reading the Users", &err))?;
            Ok(Users {
                users: (listed.into_iter())
                    .map(|user| User {
                        id: user.id,
                        display_name: user.name,
                        ..User::default()
                    })
                    .collect(),
                ..Users::default()
            })
        })
    }

    fn list_requesters<'a>(
        &'a self,
        asked: &'a RequestersAsked,
    ) -> BoxFuture<'a, Result<Requesters, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::ListRequesters)?;
            let mut titles = Vec::with_capacity(asked.titles.len());
            // ponytail: a few queries per title; read a page's titles together if ticks get slow.
            for title in &asked.titles {
                let requesters = self.requesters(&automation, title).await?;
                titles.push(TitleRequesters {
                    title_ref: MessageField::some(title.clone()),
                    requested: requesters.is_some(),
                    user_ids: requesters.unwrap_or_default(),
                    ..TitleRequesters::default()
                });
            }
            Ok(Requesters {
                titles,
                ..Requesters::default()
            })
        })
    }

    fn delete_media<'a>(
        &'a self,
        asked: &'a DeleteAsked,
    ) -> BoxFuture<'a, Result<DeleteStarted, KernelRefused>> {
        Box::pin(async move {
            let automation = self.check(Method::DeleteMedia)?;
            let title = (asked.title_ref.as_option())
                .ok_or_else(|| KernelRefused("the title to delete is missing".to_owned()))?;
            let to_delete = ToDelete::Title {
                library: title.library_id,
                title: titled(title, Target::Movie, Target::Episode)?,
            };
            let doomed = match automation.browse.doomed(to_delete).await {
                Ok(doomed) => doomed,
                Err(browse::Error::Database(err)) => {
                    return Err(self.database_refused("finding the media to delete", &err));
                }
                Err(err) => return Err(KernelRefused(err.to_string())),
            };
            // `start_delete` refuses this too, as the Scanner's own guard for every caller.
            // Checked here first so a read-only title is refused for that before the taken-along
            // check reads the Episodes' Shows and refuses it for something else.
            if let Some(folder) = doomed.read_only_folder() {
                return Err(KernelRefused(
                    scanner::Error::ReadOnly(folder.to_owned()).to_string(),
                ));
            }
            let taken_along: Vec<TitleRef> = (doomed.taken_along.iter())
                .map(|episode| {
                    let kind = TitleRefKind::TITLE_REF_KIND_EPISODE;
                    title_ref(title.library_id, kind, &episode.id)
                })
                .collect();
            (self.check_taken_along(
                &automation,
                title.library_id,
                &doomed.taken_along,
                &asked.also,
            ))
            .await?;
            let by = format!("the {} Plugin", automation.points.name(&self.plugin));
            // Not awaited: the delete waits for the Library's scan lock, which could outrun the
            // Plugin's call limit, and it goes on in a task of its own anyway (ADR 0029). On the
            // Scanner's tracker, not in the Plugin's scope, since it never calls the Plugin back,
            // so disabling the Plugin doesn't stop a delete it started (ADR 0048) and shutting
            // down waits for it. A failure is logged there, since the Plugin has had its answer.
            (automation
                .scanner
                .start_delete(doomed, by, self.plugin.clone()))
            .map_err(|err| KernelRefused(err.to_string()))?;
            Ok(DeleteStarted {
                taken_along,
                ..DeleteStarted::default()
            })
        })
    }
}

#[cfg(test)]
#[path = "automation/automation_tests.rs"]
mod tests;
