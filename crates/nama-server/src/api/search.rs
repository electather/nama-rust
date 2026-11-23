//! Finding titles: `nama.v1.SearchService` (ADR 0080).

use std::collections::HashSet;

use buffa::EnumValue;
use connectrpc::{
    ConnectError, RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream,
};
use futures_util::{
    StreamExt,
    future::{self, OptionFuture},
    stream,
};
use nama_plugin_api::{PluginId, metadata::ExternalId};
use nama_proto::nama::v1::{
    self as proto, Done, Hits, RefusalReason, SearchRequest, SearchResponse, SearchService,
    TitleStatus, search_response::Part,
};
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{Instrument as _, Span};

use crate::{
    browse::{Browse, PersonPage},
    library::{
        Candidate, Downloaded, Found, FoundEpisode, FoundMovie, FoundShow, ImageSources, OnServer,
        Role, ShownImages, TitleImage, TitleKind,
    },
    requests::{self, Requests, ShowToRequest, SourceAnswer, Sourced, Unavailable, year_of},
    users::User,
};

use super::{
    browse::{Fields, headshot_url, refused},
    caller, database_failed, external_id_to_proto, proto_number, refuse,
    requests::{searched_show_to_proto, searched_to_proto},
    title_kind_from_proto,
};

/// How many results a batch holds when the request doesn't say.
const DEFAULT_LIMIT: usize = 20;

/// The most results a batch holds, whatever the request says.
const MAX_LIMIT: usize = 100;

/// `nama.v1.SearchService`: finding titles, for any signed-in User.
#[derive(Clone)]
pub(super) struct SearchServiceHandler {
    pub(super) browse: Browse,
    pub(super) requests: Requests,
    pub(super) images: ImageSources,
}

#[expect(
    refining_impl_trait,
    reason = "plain `async fn` returning the owned stream is the form the generated trait \
              documents; the refined signature isn't visible outside this crate"
)]
impl SearchService for SearchServiceHandler {
    async fn search(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<'_, SearchRequest>,
    ) -> ServiceResult<ServiceStream<SearchResponse>> {
        let viewer = caller(&ctx)?.user.clone();
        let request = request.to_owned_message();
        let query = searched_query(&request.query)?;
        let kind = searched_kind(request.kind)?;
        let limit = usize::try_from(request.limit)
            .ok()
            .filter(|&limit| limit > 0)
            .map_or(DEFAULT_LIMIT, |limit| limit.min(MAX_LIMIT));
        let found = self.browse.search(query, kind, limit).await;
        let found = found.map_err(database_failed)?;
        let people = self.browse.people_named(query, kind).await;
        let people = people.map_err(refused(Fields::NONE))?;
        let images = self.images.shown().await.map_err(database_failed)?;
        let sent = Sent {
            movies: library_ids(&found, TitleKind::Movie),
            shows: library_ids(&found, TitleKind::Show),
        };
        let people = (people.into_iter()).map(|page| proto::Hit {
            title: person_hit(page, &images).into(),
            status: TitleStatus::TITLE_STATUS_AVAILABLE.into(),
            ..Default::default()
        });
        let library = Hits {
            hits: (found.into_iter())
                .map(|found| hit_to_proto(found, &images))
                .chain(people)
                .collect(),
            ..Default::default()
        };
        let searched = Searched {
            viewer,
            query: query.to_owned(),
            kind,
            limit,
        };
        // Batches go through a channel, so each goes out as soon as it is ready, the title
        // source's without waiting for the Search sources asked beside it. The work is polled with
        // the stream rather than spawned, so a client that leaves stops the sources being asked.
        let (parts, received) = mpsc::unbounded_channel();
        let handler = self.clone();
        let sending = async move { handler.after_library(searched, sent, images, &parts).await };
        let sending =
            stream::once(sending.instrument(Span::current())).filter_map(|()| future::ready(None));
        let received = stream::unfold(received, |mut received| async move {
            Some((received.recv().await?, received))
        });
        let stream: ServiceStream<SearchResponse> = Box::pin(
            stream::once(future::ready(Ok(response(library))))
                .chain(stream::select(received, sending)),
        );
        Response::ok(stream)
    }
}

/// What a search asks, which every batch after the Libraries' answers.
struct Searched {
    viewer: User,
    query: String,
    kind: Option<TitleKind>,
    limit: usize,
}

/// The ids of every title of each kind a batch sent, which a later batch leaves out. Kept apart by
/// kind, since a title source may give a Movie and a Show one id.
#[derive(Default)]
struct Sent {
    movies: HashSet<ExternalId>,
    shows: HashSet<ExternalId>,
}

impl Sent {
    fn of(&mut self, kind: TitleKind) -> &mut HashSet<ExternalId> {
        match kind {
            TitleKind::Movie => &mut self.movies,
            TitleKind::Show => &mut self.shows,
        }
    }

    fn has_any(&self, kind: TitleKind, ids: &[ExternalId]) -> bool {
        let sent = match kind {
            TitleKind::Movie => &self.movies,
            TitleKind::Show => &self.shows,
        };
        ids.iter().any(|id| sent.contains(id))
    }
}

/// Where the stream's parts after the Libraries' batch go. A send fails only once the client has
/// gone, when nobody needs the rest.
type Parts = UnboundedSender<Result<SearchResponse, ConnectError>>;

impl SearchServiceHandler {
    /// Sends what the stream sends after the Libraries' batch to `parts`: the title source's batch
    /// of each kind, then each Search source's, then `Done`. A failure other than those `Done`
    /// names ends it with its error, after the batches before it.
    async fn after_library(
        &self,
        searched: Searched,
        sent: Sent,
        images: ShownImages,
        parts: &Parts,
    ) {
        let deliver = |hits: Hits| drop(parts.send(Ok(response(hits))));
        let done = self
            .batches_after_library(searched, sent, &images, deliver)
            .await;
        drop(parts.send(done.map(response)));
    }

    /// Hands each batch [`Self::after_library`] sends to `deliver` as it is ready, and returns
    /// `Done`, which flags a kind the title source couldn't search and names each Search source
    /// that failed. The title source and the Search sources are asked at once.
    async fn batches_after_library(
        &self,
        searched: Searched,
        mut sent: Sent,
        images: &ShownImages,
        deliver: impl Fn(Hits),
    ) -> Result<Done, ConnectError> {
        let sources =
            (self.requests).ask_search_sources(&searched.query, searched.kind, searched.limit);
        let title_source = self.title_source_batches(&searched, &mut sent, images, &deliver);
        let (title_source_unavailable, sources) = future::join(title_source, sources).await;
        let mut done = Done {
            title_source_unavailable: title_source_unavailable?,
            ..Default::default()
        };
        let batches = SourceBatches::new(self, &searched, images).await;
        for SourceAnswer { plugin, titles } in sources {
            match titles {
                Some(titles) => deliver(batches.batch(&plugin, titles, &mut sent).await?),
                None => done.failed_search_sources.push(plugin.to_string()),
            }
        }
        Ok(done)
    }

    /// Hands the title source's batch of each kind `searched` asks for to `deliver`, Movies first,
    /// adding each title it sends to `sent`. Returns whether the title source couldn't search a
    /// kind.
    async fn title_source_batches(
        &self,
        searched: &Searched,
        sent: &mut Sent,
        images: &ShownImages,
        deliver: &impl Fn(Hits),
    ) -> Result<bool, ConnectError> {
        let Searched {
            viewer,
            query,
            kind,
            limit,
        } = searched;
        let asks_for = |of| kind.is_none_or(|kind| kind == of);
        let movies = OptionFuture::from(
            asks_for(TitleKind::Movie)
                .then(|| (self.requests).search_to_request(viewer, query, &sent.movies)),
        );
        let shows = OptionFuture::from(
            asks_for(TitleKind::Show)
                .then(|| (self.requests).search_shows_to_request(viewer, query, &sent.shows)),
        );
        let (movies, shows) = future::join(movies, shows).await;
        for found in first(movies.as_ref(), *limit) {
            let ids = found.movie.ids.iter().chain([found.id.clone()]);
            sent.of(TitleKind::Movie).extend(ids);
        }
        for ShowToRequest { hit, .. } in first(shows.as_ref(), *limit) {
            let ids = hit.candidate.ids.iter().chain([hit.id.clone()]);
            sent.of(TitleKind::Show).extend(ids);
        }
        let mut unavailable = false;
        for answered in [
            batch(movies, *limit, |movie| movie_to_request_hit(movie, images)),
            batch(shows, *limit, |show| show_to_request_hit(show, images)),
        ] {
            match answered {
                Ok(hits) => hits.into_iter().for_each(deliver),
                Err(requests::Error::Unavailable) => unavailable = true,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(unavailable)
    }
}

/// What each Search source's batch in one search reads, read once.
struct SourceBatches<'a> {
    handler: &'a SearchServiceHandler,
    viewer: &'a User,
    images: &'a ShownImages,
    limit: usize,
    /// The namespaces the viewer may request a Movie under.
    movie_namespaces: Vec<String>,
    /// The namespaces the viewer may request a Show under.
    show_namespaces: Vec<String>,
}

impl<'a> SourceBatches<'a> {
    async fn new(
        handler: &'a SearchServiceHandler,
        searched: &'a Searched,
        images: &'a ShownImages,
    ) -> Self {
        let viewer = &searched.viewer;
        let requests = &handler.requests;
        let (movie_namespaces, show_namespaces) = future::join(
            requests.requestable_namespaces(viewer, TitleKind::Movie),
            requests.requestable_namespaces(viewer, TitleKind::Show),
        )
        .await;
        Self {
            handler,
            viewer,
            images,
            limit: searched.limit,
            movie_namespaces,
            show_namespaces,
        }
    }

    /// The batch of the titles the Search source `plugin` found, as the viewer sees them
    /// (ADR 0083): those no earlier batch in `sent` sent, by status, then in the source's order,
    /// at most `limit`, each added to `sent`. One the viewer may neither watch nor request is left
    /// out.
    async fn batch(
        &self,
        plugin: &PluginId,
        titles: Vec<Sourced>,
        sent: &mut Sent,
    ) -> Result<Hits, ConnectError> {
        let mut in_batch = Sent::default();
        let mut shown = Vec::new();
        // ponytail: a Library lookup and a Request lookup per title, one after another, of at most
        // a source's first 100.
        for title in titles.into_iter().take(MAX_LIMIT) {
            let (kind, ids) = (title.kind, title.ids.iter().collect::<Vec<_>>());
            let seen = |by: &[ExternalId]| sent.has_any(kind, by) || in_batch.has_any(kind, by);
            if seen(&ids) {
                continue;
            }
            // Checked again by every id its Library knows it by, since an earlier batch may have
            // sent it by one the source didn't give.
            let Some((hit, known_by)) = self.hit(plugin, title, ids).await? else {
                continue;
            };
            if !seen(&known_by) {
                in_batch.of(kind).extend(known_by.iter().cloned());
                shown.push((hit, kind, known_by));
            }
        }
        shown.sort_by_key(|(hit, ..)| hit.status.to_i32());
        shown.truncate(self.limit);
        for (_, kind, ids) in &shown {
            sent.of(*kind).extend(ids.iter().cloned());
        }
        Ok(Hits {
            hits: shown.into_iter().map(|(hit, ..)| hit).collect(),
            ..Default::default()
        })
    }

    /// `title`, which the Search source `plugin` found by `ids`, as the viewer sees it, with every
    /// id it is known by: Available in the oldest Library with any of `ids`, shown as the Library
    /// has it, or else as [`Requests::unavailable`] tells, shown as the source gave it. `None` if
    /// the viewer may neither watch nor request it.
    async fn hit(
        &self,
        plugin: &PluginId,
        title: Sourced,
        mut ids: Vec<ExternalId>,
    ) -> Result<Option<(proto::Hit, Vec<ExternalId>)>, ConnectError> {
        let on_server = self.handler.browse.in_libraries(title.kind, &ids).await;
        if let Some(on_server) = on_server.map_err(refused(Fields::NONE))?.into_iter().next() {
            let (id, known) = match &on_server {
                OnServer::Movie(movie) => (&movie.id, &movie.ids),
                OnServer::Show(show) => (&show.id, &show.ids),
            };
            ids.extend(known.iter().chain([id.clone()]));
            return Ok(Some((available_hit(on_server, self.images), ids)));
        }
        let requestable = match title.kind {
            TitleKind::Movie => &self.movie_namespaces,
            TitleKind::Show => &self.show_namespaces,
        };
        let unavailable = (self.handler.requests).unavailable(self.viewer, &title, requestable);
        let unavailable = unavailable.await.map_err(database_failed)?;
        Ok(unavailable
            .map(|unavailable| (sourced_hit(plugin, title, unavailable, self.images), ids)))
    }
}

/// The response of the stream that carries `part`.
fn response(part: impl Into<Part>) -> SearchResponse {
    SearchResponse {
        part: Some(part.into()),
        ..Default::default()
    }
}

/// `query`, unless it is blank, which a search refuses.
fn searched_query(query: &str) -> Result<&str, ConnectError> {
    if query.trim().is_empty() {
        return Err(refuse(
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "the words to search for must not be blank",
        )
        .field("query")
        .into());
    }
    Ok(query)
}

/// The one kind of title `kind` asks for, or `None` for every kind when it's unset.
fn searched_kind(kind: EnumValue<proto::TitleKind>) -> Result<Option<TitleKind>, ConnectError> {
    if kind == EnumValue::Known(proto::TitleKind::TITLE_KIND_UNSPECIFIED) {
        return Ok(None);
    }
    title_kind_from_proto(kind).map(Some)
}

/// A title a search of the Libraries found, which the caller can watch.
fn hit_to_proto(found: Found, images: &ShownImages) -> proto::Hit {
    let title = match found {
        Found::Movie(movie) => movie_hit(movie, images).into(),
        Found::Show(show) => show_hit(show, images).into(),
        Found::Episode(episode) => episode_hit(episode, images).into(),
    };
    proto::Hit {
        title,
        status: TitleStatus::TITLE_STATUS_AVAILABLE.into(),
        ..Default::default()
    }
}

/// Every id of each title of `kind` in `found`, which the later batches of that kind leave out.
fn library_ids(found: &[Found], kind: TitleKind) -> HashSet<ExternalId> {
    (found.iter())
        .filter_map(|found| match (found, kind) {
            (Found::Movie(movie), TitleKind::Movie) => {
                Some(movie.ids.iter().chain([movie.id.clone()]))
            }
            (Found::Show(show), TitleKind::Show) => Some(show.ids.iter().chain([show.id.clone()])),
            _ => None,
        })
        .flatten()
        .collect()
}

/// The first `limit` of what the title source's search of one kind `searched` found, which its
/// batch sends; none if it wasn't asked or failed.
fn first<T>(
    searched: Option<&Result<Option<Vec<T>>, requests::Error>>,
    limit: usize,
) -> impl Iterator<Item = &T> {
    let found = match searched {
        Some(Ok(Some(found))) => found.as_slice(),
        _ => &[],
    };
    found.iter().take(limit)
}

/// The first `limit` of what the title source's search of one kind `searched` found, ranked, as a
/// batch; `None` if it wasn't asked, or the caller may not request that kind.
fn batch<T>(
    searched: Option<Result<Option<Vec<T>>, requests::Error>>,
    limit: usize,
    to_hit: impl FnMut(T) -> proto::Hit,
) -> Result<Option<Hits>, requests::Error> {
    let found = searched.transpose()?.flatten();
    Ok(found.map(|found| Hits {
        hits: found.into_iter().take(limit).map(to_hit).collect(),
        ..Default::default()
    }))
}

/// A title on the Server a Search source found, shown as the Library has it.
fn available_hit(on_server: OnServer, images: &ShownImages) -> proto::Hit {
    let title = match on_server {
        OnServer::Movie(movie) => movie_hit(movie, images).into(),
        OnServer::Show(show) => show_hit(show, images).into(),
    };
    proto::Hit {
        title,
        status: TitleStatus::TITLE_STATUS_AVAILABLE.into(),
        ..Default::default()
    }
}

/// A title the Search source `plugin` found that no Library has, shown as the source gave it, its
/// Poster through `plugin`, unless the title has a Custom Image.
fn sourced_hit(
    plugin: &PluginId,
    title: Sourced,
    unavailable: Unavailable,
    images: &ShownImages,
) -> proto::Hit {
    let (status, id, request) = match unavailable {
        Unavailable::Requested { id, request } => (
            TitleStatus::TITLE_STATUS_REQUESTED,
            id,
            request.map(|request| *request),
        ),
        Unavailable::Requestable(id) => (TitleStatus::TITLE_STATUS_NOT_ON_THIS_SERVER, id, None),
    };
    let downloaded = (title.poster.as_deref()).map(|path| Downloaded::Offered { plugin, path });
    let role = match title.kind {
        TitleKind::Movie => TitleImage::Movie(Role::Poster),
        TitleKind::Show => TitleImage::Show(Role::Poster),
    };
    let poster = images.of_title(&id, &title.ids, role, downloaded);
    let (id, request) = (external_id_to_proto(&id).into(), request.into());
    let shown = match title.kind {
        TitleKind::Movie => proto::MovieHit {
            id,
            title: title.title,
            year: title.year,
            poster_url: poster.url,
            poster_thumbhash: poster.thumbhash,
            request,
            ..Default::default()
        }
        .into(),
        TitleKind::Show => proto::ShowHit {
            id,
            title: title.title,
            year: title.year,
            poster_url: poster.url,
            poster_thumbhash: poster.thumbhash,
            request,
            ..Default::default()
        }
        .into(),
    };
    proto::Hit {
        title: shown,
        status: status.into(),
        ..Default::default()
    }
}

/// How close a title the title source found is to being on the Server.
fn status(library: Option<i64>, requested: bool) -> TitleStatus {
    match (library, requested) {
        (Some(_), _) => TitleStatus::TITLE_STATUS_AVAILABLE,
        (None, true) => TitleStatus::TITLE_STATUS_REQUESTED,
        (None, false) => TitleStatus::TITLE_STATUS_NOT_ON_THIS_SERVER,
    }
}

/// A Movie the title source found, with how close it is to being on the Server.
fn movie_to_request_hit(found: requests::Found<Candidate>, images: &ShownImages) -> proto::Hit {
    proto::Hit {
        status: status(found.library, found.request.is_some()).into(),
        title: searched_to_proto(found, images).into(),
        ..Default::default()
    }
}

/// A Show the title source found, with how close it is to being on the Server.
fn show_to_request_hit(found: ShowToRequest, images: &ShownImages) -> proto::Hit {
    let status = status(found.library, found.requested());
    let show = proto::ShowHit {
        library_id: found.library,
        request: found.request.into(),
        ..searched_show_to_proto(found.hit, images)
    };
    proto::Hit {
        status: status.into(),
        title: show.into(),
        ..Default::default()
    }
}

/// The Movies among `titles`, each as a hit that opens it in its Library, such as a title page's
/// Similar titles.
pub(super) fn movie_hits(titles: Vec<OnServer>, images: &ShownImages) -> Vec<proto::MovieHit> {
    (titles.into_iter())
        .filter_map(|title| match title {
            OnServer::Movie(movie) => Some(movie_hit(movie, images)),
            OnServer::Show(_) => None,
        })
        .collect()
}

/// As [`movie_hits`], for the Shows among `titles`.
pub(super) fn show_hits(titles: Vec<OnServer>, images: &ShownImages) -> Vec<proto::ShowHit> {
    (titles.into_iter())
        .filter_map(|title| match title {
            OnServer::Show(show) => Some(show_hit(show, images)),
            OnServer::Movie(_) => None,
        })
        .collect()
}

pub(super) fn movie_hit(movie: FoundMovie, images: &ShownImages) -> proto::MovieHit {
    let downloaded = movie.poster.as_ref().map(Downloaded::stored);
    let role = TitleImage::Movie(Role::Poster);
    let poster = images.of_title(&movie.id, &movie.ids, role, downloaded);
    proto::MovieHit {
        id: external_id_to_proto(&movie.id).into(),
        title: movie.title,
        year: movie.release_date.as_deref().and_then(year_of),
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        library_id: Some(movie.library),
        ..Default::default()
    }
}

pub(super) fn show_hit(show: FoundShow, images: &ShownImages) -> proto::ShowHit {
    let downloaded = show.poster.as_ref().map(Downloaded::stored);
    let role = TitleImage::Show(Role::Poster);
    let poster = images.of_title(&show.id, &show.ids, role, downloaded);
    proto::ShowHit {
        id: external_id_to_proto(&show.id).into(),
        title: show.title,
        year: show.first_air_date.as_deref().and_then(year_of),
        poster_url: poster.url,
        poster_thumbhash: poster.thumbhash,
        library_id: Some(show.library),
        ..Default::default()
    }
}

/// A Person a search found, as clients see them.
fn person_hit(page: PersonPage, images: &ShownImages) -> proto::PersonHit {
    proto::PersonHit {
        headshot_url: headshot_url(&page, images),
        id: external_id_to_proto(&page.id).into(),
        name: page.name,
        ..Default::default()
    }
}

fn episode_hit(episode: FoundEpisode, images: &ShownImages) -> proto::EpisodeHit {
    let still = images.downloaded(episode.still.as_ref().map(Downloaded::stored));
    let downloaded = episode.show_poster.as_ref().map(Downloaded::stored);
    let role = TitleImage::Show(Role::Poster);
    let show_poster = images.of_title(&episode.show, &episode.show_ids, role, downloaded);
    proto::EpisodeHit {
        library_id: episode.library,
        id: external_id_to_proto(&episode.id).into(),
        show_id: external_id_to_proto(&episode.show).into(),
        show_title: episode.show_title,
        season_number: proto_number(episode.season),
        episode_number: proto_number(episode.number),
        title: episode.title,
        still_url: still.url,
        still_thumbhash: still.thumbhash,
        show_poster_url: show_poster.url,
        show_poster_thumbhash: show_poster.thumbhash,
        ..Default::default()
    }
}
