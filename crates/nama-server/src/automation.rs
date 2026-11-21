//! Automation: the Kernel's functions any Plugin calls (ADR 0048, ADR 0071), each by name, with
//! the Plugin Permission it needs checked before its request is read.

mod sharing;
mod store;
#[cfg(test)]
mod testing;

use std::{
    collections::{HashMap, HashSet},
    future,
    sync::{Arc, Weak},
    time::Duration,
};

use buffa::{EnumValue, Message, MessageField};
use nama_plugin_api::{
    BoxFuture, PluginId,
    automation::{
        BackUpAsked, BackedUp, DeleteAsked, DeleteStarted, ExpiredSignInsAsked,
        ExpiredSignInsDeleted, ForgetGonePluginsAsked, ForgetHomeRowsAsked, GonePluginsForgotten,
        HomeRowsForgotten, Kernel, KernelRefused, Libraries, LibrariesAsked, Library, LibraryKind,
        MetadataRefreshed, Permission, RefreshMetadataAsked, Requesters, RequestersAsked,
        StoreEntry, StoreKey, StoreValue, Stored, Title, TitleRef, TitleRefKind, TitleRequesters,
        TitleWatchStates, Titles, TitlesAsked, User, Users, UsersAsked, WatchState, WatchStates,
        WatchStatesAsked, Watchlist, WatchlistAsked, WatchlistTitle,
    },
    contract,
    fulfiller::TitleId,
    metadata::{ExternalId, ExternalIds},
};
use sqlx::SqliteConnection;
use tracing::error;

use crate::{
    backup::Backups,
    connections,
    db::Database,
    home_rows,
    library::{self, Contents, ItemKind, LibraryError, TitleKind, TitleTarget, WatchStateKind},
    plugin_settings::PluginSettings,
    requests,
    scanner::{self, Scanner, ToDelete},
    users, watchlist,
};

/// The most titles one page of `list_titles` holds.
const MAX_PAGE_SIZE: usize = 1000;

/// What makes the Kernel's functions for each enabled Plugin.
///
/// The Plugins it serves hold it only weakly, since it holds them in turn through the Scanner,
/// `plugins` and `Users`: whoever starts the Server holds it, until the Plugins have stopped.
pub(crate) struct Automation {
    db: Arc<Database>,
    scanner: Scanner,
    plugins: PluginSettings,
    users: users::Users,
    backups: Backups,
}

impl Automation {
    /// Serves the Kernel's functions to each Plugin enabled in `plugins`.
    pub(crate) fn serve(
        db: Arc<Database>,
        scanner: Scanner,
        plugins: PluginSettings,
        users: users::Users,
        backups: Backups,
    ) -> Arc<Self> {
        let automation = Arc::new(Self {
            db,
            scanner,
            plugins,
            users,
            backups,
        });
        let kernels = Arc::downgrade(&automation);
        automation
            .plugins
            .extension_points()
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

/// A Kernel function's encoded response, or why it refused.
type Answer = BoxFuture<'static, Result<Vec<u8>, KernelRefused>>;

/// One Kernel function: its name, the Plugin Permission it needs, and how it answers a request.
struct Function {
    name: &'static str,
    needs: Permission,
    answer: fn(Caller, Vec<u8>) -> Answer,
}

/// Every Kernel function, as `automation.proto` lists them: the one place that decides which
/// Plugin Permission each needs.
const FUNCTIONS: &[Function] = &[
    Function {
        name: "store_get",
        needs: Permission::PERMISSION_STORE,
        answer: |caller, request| typed(caller, request, Caller::store_get),
    },
    Function {
        name: "store_put",
        needs: Permission::PERMISSION_STORE,
        answer: |caller, request| typed(caller, request, Caller::store_put),
    },
    Function {
        name: "store_delete",
        needs: Permission::PERMISSION_STORE,
        answer: |caller, request| typed(caller, request, Caller::store_delete),
    },
    Function {
        name: "list_libraries",
        needs: Permission::PERMISSION_READ_LIBRARIES,
        answer: |caller, request| typed(caller, request, Caller::list_libraries),
    },
    Function {
        name: "list_titles",
        needs: Permission::PERMISSION_READ_LIBRARIES,
        answer: |caller, request| typed(caller, request, Caller::list_titles),
    },
    Function {
        name: "list_watch_states",
        needs: Permission::PERMISSION_READ_WATCH_STATES,
        answer: |caller, request| typed(caller, request, Caller::list_watch_states),
    },
    Function {
        name: "list_users",
        needs: Permission::PERMISSION_READ_USERS,
        answer: |caller, request| typed(caller, request, Caller::list_users),
    },
    Function {
        name: "list_requesters",
        needs: Permission::PERMISSION_READ_REQUESTS,
        answer: |caller, request| typed(caller, request, Caller::list_requesters),
    },
    Function {
        name: "list_watchlist",
        needs: Permission::PERMISSION_READ_WATCHLISTS,
        answer: |caller, request| typed(caller, request, Caller::list_watchlist),
    },
    Function {
        name: "delete_media",
        needs: Permission::PERMISSION_DELETE_MEDIA,
        answer: |caller, request| typed(caller, request, Caller::delete_media),
    },
    Function {
        name: "forget_home_rows",
        needs: Permission::PERMISSION_FORGET_HOME_ROWS,
        answer: |caller, request| typed(caller, request, Caller::forget_home_rows),
    },
    Function {
        name: "delete_expired_sign_ins",
        needs: Permission::PERMISSION_DELETE_EXPIRED_SIGN_INS,
        answer: |caller, request| typed(caller, request, Caller::delete_expired_sign_ins),
    },
    Function {
        name: "forget_gone_plugins",
        needs: Permission::PERMISSION_FORGET_GONE_PLUGINS,
        answer: |caller, request| typed(caller, request, Caller::forget_gone_plugins),
    },
    Function {
        name: "refresh_metadata",
        needs: Permission::PERMISSION_REFRESH_METADATA,
        answer: |caller, request| typed(caller, request, Caller::refresh_metadata),
    },
    Function {
        name: "list_titles_sharing",
        needs: Permission::PERMISSION_READ_LIBRARIES,
        answer: |caller, request| typed(caller, request, Caller::list_titles_sharing),
    },
    Function {
        name: "back_up",
        needs: Permission::PERMISSION_BACK_UP,
        answer: |caller, request| typed(caller, request, Caller::back_up),
    },
];

/// Decodes `request`, has `handler` answer it for `caller`, and encodes its response.
fn typed<Req, Resp, Handled>(
    caller: Caller,
    request: Vec<u8>,
    handler: fn(Caller, Req) -> Handled,
) -> Answer
where
    Req: Message,
    Resp: Message,
    Handled: Future<Output = Result<Resp, KernelRefused>> + Send + 'static,
{
    Box::pin(async move {
        let request = Req::decode_from_slice(&request).map_err(|_unreadable| {
            KernelRefused("the Kernel couldn't read the request".to_owned())
        })?;
        Ok(handler(caller, request).await?.encode_to_vec())
    })
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
        Permission::PERMISSION_READ_WATCHLISTS => "Read Watchlists",
        Permission::PERMISSION_FORGET_HOME_ROWS => "Forget Home rows",
        Permission::PERMISSION_DELETE_EXPIRED_SIGN_INS => "Delete expired sign-ins",
        Permission::PERMISSION_FORGET_GONE_PLUGINS => "Forget removed Plugins",
        Permission::PERMISSION_REFRESH_METADATA => "Refresh metadata",
        Permission::PERMISSION_BACK_UP => "Back up",
    }
}

struct PluginKernel {
    /// Held weakly: see [`Automation`].
    automation: Weak<Automation>,
    plugin: PluginId,
    granted: HashSet<Permission>,
}

impl Kernel for PluginKernel {
    fn call(
        &self,
        function: &str,
        request: Vec<u8>,
    ) -> BoxFuture<'_, Result<Vec<u8>, KernelRefused>> {
        self.dispatch(function, request)
            .unwrap_or_else(|refused| Box::pin(future::ready(Err(refused))))
    }
}

impl PluginKernel {
    /// The answer to `function`, once the Plugin was granted the Plugin Permission it needs:
    /// checked before the request is read, so no handler checks one itself.
    fn dispatch(&self, function: &str, request: Vec<u8>) -> Result<Answer, KernelRefused> {
        let found = (FUNCTIONS.iter())
            .find(|found| found.name == function)
            .ok_or_else(|| KernelRefused(format!("the Kernel has no function {function}")))?;
        if !self.granted.contains(&found.needs) {
            return Err(KernelRefused(format!(
                "the Admin hasn't granted {} the {} Permission",
                self.plugin,
                permission_name(found.needs)
            )));
        }
        let automation = (self.automation.upgrade())
            .ok_or_else(|| KernelRefused("the Server is stopping".to_owned()))?;
        let caller = Caller {
            automation,
            plugin: self.plugin.clone(),
        };
        Ok((found.answer)(caller, request))
    }
}

/// The Plugin a Kernel function answers, and the Automation answering it.
struct Caller {
    automation: Arc<Automation>,
    plugin: PluginId,
}

impl Caller {
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
    async fn list_titles(self, asked: TitlesAsked) -> Result<Titles, KernelRefused> {
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
        let found = library::Libraries::new(Arc::clone(&self.automation.db))
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
        let mut listed = Contents::new(Arc::clone(&self.automation.db))
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
                added_at_ms: listed.added_at.saturating_mul(1000),
                genres: listed.genres,
                content_rating: listed.content_rating,
                show_id: listed.show_id.as_ref().map(title_id).into(),
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
        titles: &[TitleRef],
    ) -> Result<Vec<(WatchStateKind, ExternalIds)>, KernelRefused> {
        let contents = Contents::new(Arc::clone(&self.automation.db));
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
    async fn requesters(&self, title: &TitleRef) -> Result<Option<Vec<i64>>, KernelRefused> {
        let contents = Contents::new(Arc::clone(&self.automation.db));
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
        let refused = |err| self.database_refused("reading the Requests", &err);
        let mut conn = self
            .automation
            .db
            .reader()
            .acquire()
            .await
            .map_err(refused)?;
        (requests::who_requested(&mut conn, &ids, season).await).map_err(refused)
    }

    /// What a Plugin is told of a delete the Scanner refused, or failed to start.
    fn delete_refused(&self, err: scanner::Error) -> KernelRefused {
        match err {
            scanner::Error::Database(err) => {
                self.database_refused("finding the media to delete", &err)
            }
            scanner::Error::TakesAlongUnnamed(taken_along) => {
                let named: Vec<String> = (taken_along.iter())
                    .map(|taken| format!("{} ({})", taken.name(), taken.episode.title))
                    .collect();
                KernelRefused(format!(
                    "the title's files also hold {}; name each in `also` to delete it too, or \
                     keep the files",
                    named.join(", ")
                ))
            }
            err @ (scanner::Error::Library(_)
            | scanner::Error::Walk(_)
            | scanner::Error::Cancelled
            | scanner::Error::ReadOnly(_)
            | scanner::Error::Requests(_)
            | scanner::Error::NotFound(_)) => KernelRefused(err.to_string()),
        }
    }

    async fn store_get(self, key: StoreKey) -> Result<StoreValue, KernelRefused> {
        let value = store::get(&self.automation.db, &self.plugin, key.user_id, &key.key)
            .await
            .map_err(|err| self.store_refused(&err))?;
        Ok(StoreValue {
            value,
            ..StoreValue::default()
        })
    }

    async fn store_put(self, entry: StoreEntry) -> Result<Stored, KernelRefused> {
        let (db, plugin) = (&self.automation.db, &self.plugin);
        store::put(db, plugin, entry.user_id, entry.key, entry.value)
            .await
            .map_err(|err| self.store_refused(&err))?;
        Ok(Stored::default())
    }

    async fn store_delete(self, key: StoreKey) -> Result<Stored, KernelRefused> {
        store::delete(&self.automation.db, &self.plugin, key.user_id, key.key)
            .await
            .map_err(|err| self.store_refused(&err))?;
        Ok(Stored::default())
    }

    async fn list_libraries(self, _asked: LibrariesAsked) -> Result<Libraries, KernelRefused> {
        let listed = library::Libraries::new(Arc::clone(&self.automation.db))
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
    }

    async fn list_watch_states(
        self,
        asked: WatchStatesAsked,
    ) -> Result<WatchStates, KernelRefused> {
        let titles = self.every_id(&asked.titles).await?;
        let states = (library::WatchStates::new(Arc::clone(&self.automation.db)))
            .of_titles(asked.user_id, &titles)
            .await
            .map_err(|err| self.database_refused("reading the Watch States", &err))?;
        Ok(WatchStates {
            titles: (asked.titles.into_iter().zip(states))
                .map(|(title, states)| TitleWatchStates {
                    title_ref: MessageField::some(title),
                    states: (states.into_iter())
                        .map(|(user, state)| watch_state_to_proto(user, state))
                        .collect(),
                    ..TitleWatchStates::default()
                })
                .collect(),
            ..WatchStates::default()
        })
    }

    async fn list_users(self, _asked: UsersAsked) -> Result<Users, KernelRefused> {
        let listed = (self.automation.users.list())
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
    }

    async fn list_requesters(self, asked: RequestersAsked) -> Result<Requesters, KernelRefused> {
        let mut titles = Vec::with_capacity(asked.titles.len());
        // ponytail: a few queries per title; read a page's titles together if Job Runs get slow.
        for title in asked.titles {
            let requesters = self.requesters(&title).await?;
            titles.push(TitleRequesters {
                title_ref: MessageField::some(title),
                requested: requesters.is_some(),
                user_ids: requesters.unwrap_or_default(),
                ..TitleRequesters::default()
            });
        }
        Ok(Requesters {
            titles,
            ..Requesters::default()
        })
    }

    async fn list_watchlist(self, asked: WatchlistAsked) -> Result<Watchlist, KernelRefused> {
        let listed = watchlist::list(&self.automation.db, asked.user_id)
            .await
            .map_err(|err| self.database_refused("reading the Watchlist", &err))?;
        Ok(Watchlist {
            titles: (listed.into_iter())
                .map(|(kind, id)| WatchlistTitle {
                    kind: match kind {
                        TitleKind::Movie => contract::TitleKind::TITLE_KIND_MOVIE,
                        TitleKind::Show => contract::TitleKind::TITLE_KIND_SHOW,
                    }
                    .into(),
                    id: MessageField::some(title_id(&id)),
                    ..WatchlistTitle::default()
                })
                .collect(),
            ..Watchlist::default()
        })
    }

    async fn delete_media(self, asked: DeleteAsked) -> Result<DeleteStarted, KernelRefused> {
        let title = (asked.title_ref.as_option())
            .ok_or_else(|| KernelRefused("the title to delete is missing".to_owned()))?;
        let library = title.library_id;
        let target = ToDelete::Title {
            library,
            title: titled(title, TitleTarget::Movie, TitleTarget::Episode)?,
        };
        // One naming no Episode of the Library names nothing a delete takes along.
        let also = (asked.also.iter())
            .filter(|named| named.library_id == library)
            .filter_map(|named| titled(named, |_| None, Some).ok().flatten())
            .collect();
        let by = format!(
            "the {} Plugin",
            self.automation
                .plugins
                .extension_points()
                .name(&self.plugin)
        );
        // Only started: the delete waits for the Library's scan lock, which could outrun the
        // Plugin's call limit, and it goes on in a task of its own anyway (ADR 0029). On the
        // Scanner's tracker, not in the Plugin's scope, since it never calls the Plugin back,
        // so disabling the Plugin doesn't stop a delete it started (ADR 0048) and shutting
        // down waits for it. A failure is logged there, since the Plugin has had its answer.
        let started = (self.automation.scanner)
            .start_delete(target, also, by, self.plugin.clone())
            .await;
        let taken_along = (started.map_err(|err| self.delete_refused(err))?.iter())
            .map(|taken| {
                let kind = TitleRefKind::TITLE_REF_KIND_EPISODE;
                title_ref(library, kind, &taken.episode.id)
            })
            .collect();
        Ok(DeleteStarted {
            taken_along,
            ..DeleteStarted::default()
        })
    }

    async fn forget_home_rows(
        self,
        asked: ForgetHomeRowsAsked,
    ) -> Result<HomeRowsForgotten, KernelRefused> {
        let unvisited_for = at_least_a_day(asked.days)?;
        let users = home_rows::forget_unvisited(&self.automation.db, unvisited_for)
            .await
            .map_err(|err| self.database_refused("forgetting Home rows", &err))?;
        Ok(HomeRowsForgotten {
            users,
            ..HomeRowsForgotten::default()
        })
    }

    async fn delete_expired_sign_ins(
        self,
        _asked: ExpiredSignInsAsked,
    ) -> Result<ExpiredSignInsDeleted, KernelRefused> {
        let deleted = (self.automation.users.delete_expired())
            .await
            .map_err(|err| self.database_refused("deleting expired sign-ins", &err))?;
        Ok(ExpiredSignInsDeleted {
            devices: u32::try_from(deleted.devices).unwrap_or(u32::MAX),
            invites: u32::try_from(deleted.invites).unwrap_or(u32::MAX),
            ..ExpiredSignInsDeleted::default()
        })
    }

    /// Deletes, in one write, what each Plugin the Server hasn't had for `asked.days` left behind:
    /// its settings, Store entries, Personal settings, Plugin Connections, and what its title
    /// source gave.
    async fn forget_gone_plugins(
        self,
        asked: ForgetGonePluginsAsked,
    ) -> Result<GonePluginsForgotten, KernelRefused> {
        let gone_for = at_least_a_day(asked.days)?;
        let plugins = self.automation.plugins.clone();
        let forgotten = (self.automation.db)
            .write(move |tx| Box::pin(async move { forget_gone(tx, &plugins, gone_for).await }));
        let plugin_ids = forgotten.await.map_err(|err: sqlx::Error| {
            self.database_refused("forgetting removed Plugins", &err)
        })?;
        Ok(GonePluginsForgotten {
            plugin_ids,
            ..GonePluginsForgotten::default()
        })
    }

    /// Fetches every Matched title again by the ids its Match has. Answers only once done, which
    /// may take minutes: the time the Kernel spends answering doesn't count toward a call's limit.
    async fn refresh_metadata(
        self,
        _asked: RefreshMetadataAsked,
    ) -> Result<MetadataRefreshed, KernelRefused> {
        let refreshed = match self.automation.scanner.refresh_metadata().await {
            Ok(refreshed) => refreshed,
            Err(scanner::Error::Cancelled) => {
                return Err(KernelRefused("the Server is stopping".to_owned()));
            }
            Err(err) => return Err(self.database_refused("refreshing metadata", &err)),
        };
        Ok(MetadataRefreshed {
            titles: refreshed.titles,
            unavailable_plugin_ids: (refreshed.unavailable.iter())
                .map(ToString::to_string)
                .collect(),
            ..MetadataRefreshed::default()
        })
    }

    /// Writes a Backup where the Server settings say, refusing with why it failed, which the
    /// Admin reads in the failed Job Run: the folder is theirs to fix.
    async fn back_up(self, _asked: BackUpAsked) -> Result<BackedUp, KernelRefused> {
        (self.automation.backups.write().await)
            .map_err(|err| KernelRefused(format!("writing the Backup failed: {err}")))
    }
}

/// Deletes, in `conn`'s write, what each Plugin the Server hasn't had for `gone_for` left behind,
/// each table through its owner, what its title source gave through [`Contents`], and returns
/// their ids.
async fn forget_gone(
    conn: &mut SqliteConnection,
    plugins: &PluginSettings,
    gone_for: Duration,
) -> Result<Vec<String>, sqlx::Error> {
    let gone = plugins.take_gone(conn, gone_for).await?;
    for plugin in &gone {
        store::delete_plugin(conn, plugin).await?;
        users::delete_plugins_personal_settings(conn, plugin).await?;
        connections::delete_plugin(conn, plugin).await?;
        Contents::forget_plugin(conn, plugin).await?;
    }
    Ok(gone)
}

/// `days` as a span of time, refusing 0.
fn at_least_a_day(days: u32) -> Result<Duration, KernelRefused> {
    if days == 0 {
        return Err(KernelRefused("days must be at least 1".to_owned()));
    }
    Ok(Duration::from_hours(24 * u64::from(days)))
}

/// `id` of a Movie or Episode of the Library `library`, as a Plugin names it.
fn title_ref(library: i64, kind: TitleRefKind, id: &ExternalId) -> TitleRef {
    TitleRef {
        library_id: library,
        kind: kind.into(),
        id: MessageField::some(title_id(id)),
        ..TitleRef::default()
    }
}

/// `id` as a Plugin reads a title's id.
fn title_id(id: &ExternalId) -> TitleId {
    TitleId {
        namespace: id.namespace().to_owned(),
        value: id.value().to_owned(),
        ..TitleId::default()
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

#[cfg(test)]
#[path = "automation/automation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "automation/housekeeping_tests.rs"]
mod housekeeping_tests;
