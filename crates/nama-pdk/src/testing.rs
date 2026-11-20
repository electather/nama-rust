//! An in-process Kernel for a Plugin's native tests (ADR 0065): [`with`] installs one for a call
//! to the Plugin, which then reads its settings from it and has its HTTP requests answered by the
//! test, which reads back what the Plugin sent. It answers each Kernel function the test granted
//! from what the test gave it, and refuses the others as the Server does. Unstable while the
//! 0.x versions last. The crate's README shows a test in its section 14.

use std::{cell::RefCell, collections::HashMap, fmt};

use ::extism_pdk::{Error, HttpRequest};
use buffa::{Message, MessageField};
use nama_plugin_proto::nama::plugin::v1::{
    DeleteAsked, DeleteStarted, KernelAnswer, Libraries, LibrariesAsked, Library, Requesters,
    RequestersAsked, StoreEntry, StoreKey, StoreValue, Stored, Title, TitleRef, TitleRequesters,
    TitleWatchStates, Titles, TitlesAsked, User, Users, UsersAsked, WatchState, WatchStates,
    WatchStatesAsked, Watchlist, WatchlistAsked, WatchlistTitle, kernel_answer,
};

use crate::{Response, kernel::MAX_PAGE_SIZE};

/// Answers a Plugin's HTTP request, given with its body (empty for none); an `Err` is a host
/// that can't be reached.
type Handler = Box<dyn FnMut(&HttpRequest, &[u8]) -> Result<Response, Error>>;

/// Answers a Plugin's `delete_media` of a title, given with the titles its `also` names.
type DeleteHandler = Box<dyn FnMut(&TitleRef, &[TitleRef]) -> Result<Vec<TitleRef>, String>>;

/// What the Kernel gives a Plugin during a test: its settings, answers to its HTTP requests, and
/// the Kernel functions the test granted.
#[derive(Default)]
pub struct Kernel {
    settings: HashMap<String, String>,
    http: Option<Handler>,
    sent: Vec<Sent>,
    // Each `Option` is `None` when the Admin didn't grant the Plugin Permission its Kernel
    // functions need.
    store: Option<Store>,
    libraries: Option<Vec<Library>>,
    watch_states: Option<Vec<(TitleRef, WatchState)>>,
    users: Option<Vec<User>>,
    requesters: Option<Vec<(TitleRef, Vec<i64>)>>,
    watchlists: Option<HashMap<i64, Vec<WatchlistTitle>>>,
    delete_media: Option<DeleteHandler>,
    /// Each Library's titles, by its id, answered once `libraries` is granted.
    titles: HashMap<i64, Vec<Title>>,
    deletes: Vec<Delete>,
}

/// The values a Plugin's Store keeps, by key and User (`None` for the Plugin alone).
type Store = HashMap<(String, Option<i64>), Vec<u8>>;

/// An HTTP request a Plugin sent.
#[derive(Debug, Clone, PartialEq)]
pub struct Sent {
    /// Its URL, method and headers.
    pub request: HttpRequest,
    /// Its body, empty if it had none.
    pub body: Vec<u8>,
}

/// A `delete_media` a Plugin asked the Kernel for, whether or not it was refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Delete {
    /// The title whose media files to delete.
    pub title: TitleRef,
    /// The other titles the Plugin agreed to lose with it.
    pub also: Vec<TitleRef>,
}

impl Kernel {
    /// A Kernel with no setting set, answering no request.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The Kernel, with the setting `key` holding `value` as the Server's config would.
    #[must_use]
    pub fn setting(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_owned(), value.to_owned());
        self
    }

    /// The Kernel, answering each HTTP request with `handler`, which is given its body (empty for
    /// none). An `Err` reaches the Plugin as a host that can't be reached does: the request fails
    /// with the Plugin's own words for that.
    #[must_use]
    pub fn http(
        mut self,
        handler: impl FnMut(&HttpRequest, &[u8]) -> Result<Response, Error> + 'static,
    ) -> Self {
        self.http = Some(Box::new(handler));
        self
    }

    /// The Kernel, the Admin having granted the Plugin its Store, which starts empty. Without it,
    /// each of `kernel::store_get`, `store_put` and `store_delete` is refused.
    #[must_use]
    pub fn store(mut self) -> Self {
        self.store = Some(HashMap::new());
        self
    }

    /// The Kernel, the Admin having granted the Plugin Read Libraries, with `libraries` added to
    /// the Libraries it answers.
    #[must_use]
    pub fn libraries(mut self, libraries: impl IntoIterator<Item = Library>) -> Self {
        self.libraries.get_or_insert_default().extend(libraries);
        self
    }

    /// The Kernel, the Admin having granted the Plugin Read Libraries, with `titles` added to the
    /// Library `library_id`'s, which it pages in the order given.
    #[must_use]
    pub fn titles(mut self, library_id: i64, titles: impl IntoIterator<Item = Title>) -> Self {
        self.libraries.get_or_insert_default();
        self.titles.entry(library_id).or_default().extend(titles);
        self
    }

    /// The Kernel, the Admin having granted the Plugin Read Watch States, with `states` added to
    /// `title`'s.
    #[must_use]
    pub fn watch_states(
        mut self,
        title: &TitleRef,
        states: impl IntoIterator<Item = WatchState>,
    ) -> Self {
        let scripted = self.watch_states.get_or_insert_default();
        scripted.extend(states.into_iter().map(|state| (title.clone(), state)));
        self
    }

    /// The Kernel, the Admin having granted the Plugin Read Users, with `users` added to the Users
    /// it answers.
    #[must_use]
    pub fn users(mut self, users: impl IntoIterator<Item = User>) -> Self {
        self.users.get_or_insert_default().extend(users);
        self
    }

    /// The Kernel, the Admin having granted the Plugin Read Requests, with `title` Requested by
    /// the Users `user_ids`: none for a title whose Requesters were all deleted. A title not given
    /// here was Requested by nothing.
    #[must_use]
    pub fn requesters(mut self, title: TitleRef, user_ids: impl IntoIterator<Item = i64>) -> Self {
        let scripted = self.requesters.get_or_insert_default();
        scripted.push((title, user_ids.into_iter().collect()));
        self
    }

    /// The Kernel, the Admin having granted the Plugin Read Watchlists, with `titles` added to the
    /// User `user_id`'s Watchlist.
    #[must_use]
    pub fn watchlist(
        mut self,
        user_id: i64,
        titles: impl IntoIterator<Item = WatchlistTitle>,
    ) -> Self {
        let watchlists = self.watchlists.get_or_insert_default();
        watchlists.entry(user_id).or_default().extend(titles);
        self
    }

    /// The Kernel, the Admin having granted the Plugin Delete media, answering each
    /// `kernel::delete_media` with `handler`, which is given the title and the titles its `also`
    /// names. It answers the titles taken along, or an `Err` holding the Kernel's words refusing
    /// the delete, which reach the Plugin as the Kernel's.
    #[must_use]
    pub fn delete_media(
        mut self,
        handler: impl FnMut(&TitleRef, &[TitleRef]) -> Result<Vec<TitleRef>, String> + 'static,
    ) -> Self {
        self.delete_media = Some(Box::new(handler));
        self
    }

    /// The HTTP requests the Plugin sent, in order.
    #[must_use]
    pub fn sent(&self) -> &[Sent] {
        &self.sent
    }

    /// The deletes the Plugin asked for, in order, refused or not.
    #[must_use]
    pub fn deletes(&self) -> &[Delete] {
        &self.deletes
    }
}

impl fmt::Debug for Kernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kernel")
            .field("settings", &self.settings)
            .field("sent", &self.sent)
            .field("store", &self.store)
            .field("libraries", &self.libraries)
            .field("titles", &self.titles)
            .field("watch_states", &self.watch_states)
            .field("users", &self.users)
            .field("requesters", &self.requesters)
            .field("watchlists", &self.watchlists)
            .field("deletes", &self.deletes)
            .finish_non_exhaustive()
    }
}

thread_local! {
    static INSTALLED: RefCell<Option<Kernel>> = const { RefCell::new(None) };
}

/// Calls `call`, such as a Plugin's `fulfill`, with `kernel` as the Kernel, on this thread alone.
/// `kernel` then holds what the Plugin sent during it.
///
/// # Panics
///
/// When `call` panics, as when the Plugin sends a request `kernel` has no answer for.
#[expect(
    clippy::expect_used,
    reason = "`call` can't take the installed Kernel out of its slot"
)]
pub fn with<R>(kernel: &mut Kernel, call: impl FnOnce() -> R) -> R {
    let outer = INSTALLED.replace(Some(std::mem::take(kernel)));
    let answered = call();
    *kernel = INSTALLED
        .replace(outer)
        .expect("the Kernel stays installed through the call");
    answered
}

/// The setting `key` the installed Kernel holds.
pub(crate) fn setting(key: &str) -> Option<String> {
    installed(|kernel| kernel.settings.get(key).cloned())
}

/// The installed Kernel's answer to `request` with `body`, recording both.
#[expect(
    clippy::panic,
    reason = "an unscripted request is the test's bug, which an error the Plugin may retry or swallow would hide"
)]
pub(crate) fn http_request(request: &HttpRequest, body: &[u8]) -> Result<Response, Error> {
    installed(|kernel| {
        kernel.sent.push(Sent {
            request: request.clone(),
            body: body.to_vec(),
        });
        let Some(handler) = &mut kernel.http else {
            let method = request.method.as_deref().unwrap_or("GET");
            panic!(
                "the test scripted no HTTP answers, yet the Plugin sent {method} {}",
                request.url
            );
        };
        handler(request, body)
    })
}

/// The installed Kernel's answer to the Kernel function `name` with `request`, encoded as the
/// Kernel's `KernelAnswer`.
pub(crate) fn kernel_function(name: &str, request: &[u8]) -> Result<Vec<u8>, Error> {
    installed(|kernel| {
        let result = match kernel.answer(name, request)? {
            Ok(answer) => kernel_answer::Result::Answer(answer),
            Err(refused) => kernel_answer::Result::Refused(refused),
        };
        Ok(KernelAnswer {
            result: Some(result),
            ..KernelAnswer::default()
        }
        .encode_to_vec())
    })
}

/// A Kernel function's encoded response, or the Kernel's words refusing it.
type Answered = Result<Vec<u8>, String>;

impl Kernel {
    fn answer(&mut self, name: &str, request: &[u8]) -> Result<Answered, Error> {
        match name {
            "store_get" => typed(request, |asked| self.store_get(asked)),
            "store_put" => typed(request, |entry| self.store_put(entry)),
            "store_delete" => typed(request, |asked| self.store_delete(asked)),
            "list_libraries" => typed(request, |_: LibrariesAsked| self.list_libraries()),
            "list_titles" => typed(request, |asked| self.list_titles(&asked)),
            "list_watch_states" => typed(request, |asked| self.list_watch_states(asked)),
            "list_users" => typed(request, |_: UsersAsked| self.list_users()),
            "list_requesters" => typed(request, |asked| self.list_requesters(asked)),
            "list_watchlist" => typed(request, |asked| self.list_watchlist(&asked)),
            "delete_media" => typed(request, |asked| self.start_delete(asked)),
            _ => Err(Error::msg(format!("the Kernel has no function {name}"))),
        }
    }

    fn store_get(&self, asked: StoreKey) -> Result<StoreValue, String> {
        let store = granted(self.store.as_ref(), "Store")?;
        Ok(StoreValue {
            value: store.get(&(asked.key, asked.user_id)).cloned(),
            ..StoreValue::default()
        })
    }

    fn store_put(&mut self, entry: StoreEntry) -> Result<Stored, String> {
        let store = granted(self.store.as_mut(), "Store")?;
        store.insert((entry.key, entry.user_id), entry.value);
        Ok(Stored::default())
    }

    fn store_delete(&mut self, asked: StoreKey) -> Result<Stored, String> {
        let store = granted(self.store.as_mut(), "Store")?;
        store.remove(&(asked.key, asked.user_id));
        Ok(Stored::default())
    }

    fn list_libraries(&self) -> Result<Libraries, String> {
        Ok(Libraries {
            libraries: granted(self.libraries.clone(), "Read Libraries")?,
            ..Libraries::default()
        })
    }

    /// The page of the Library's titles `asked` names, as the Server pages them, but with the
    /// place of the next page's first title as its token.
    fn list_titles(&self, asked: &TitlesAsked) -> Result<Titles, String> {
        granted(self.libraries.as_ref(), "Read Libraries")?;
        let titles = (self.titles.get(&asked.library_id)).map_or(&[][..], Vec::as_slice);
        let from = match asked.page_token.as_str() {
            "" => 0,
            token => (token.parse().ok())
                .filter(|from| (1..titles.len()).contains(from))
                .ok_or_else(|| {
                    format!("the page token {token:?} isn't one the Kernel handed out")
                })?,
        };
        let (from, size) = match asked.page_size {
            0 => (0, titles.len()),
            size => (
                from,
                usize::try_from(size.min(MAX_PAGE_SIZE)).unwrap_or(usize::MAX),
            ),
        };
        let next = from + size;
        Ok(Titles {
            titles: titles.iter().skip(from).take(size).cloned().collect(),
            next_page_token: if next < titles.len() {
                next.to_string()
            } else {
                String::new()
            },
            ..Titles::default()
        })
    }

    fn list_watch_states(&self, asked: WatchStatesAsked) -> Result<WatchStates, String> {
        let scripted = granted(self.watch_states.as_ref(), "Read Watch States")?;
        let of_title = |title: TitleRef| {
            let states = (scripted.iter())
                .filter(|(of, state)| {
                    *of == title && asked.user_id.is_none_or(|user| user == state.user_id)
                })
                .map(|(_, state)| state.clone())
                .collect();
            TitleWatchStates {
                title_ref: MessageField::some(title),
                states,
                ..TitleWatchStates::default()
            }
        };
        Ok(WatchStates {
            titles: asked.titles.into_iter().map(of_title).collect(),
            ..WatchStates::default()
        })
    }

    fn list_users(&self) -> Result<Users, String> {
        Ok(Users {
            users: granted(self.users.clone(), "Read Users")?,
            ..Users::default()
        })
    }

    fn list_requesters(&self, asked: RequestersAsked) -> Result<Requesters, String> {
        let scripted = granted(self.requesters.as_ref(), "Read Requests")?;
        let of_title = |title: TitleRef| {
            let requested = scripted.iter().find(|(of, _)| *of == title);
            TitleRequesters {
                requested: requested.is_some(),
                user_ids: requested
                    .map(|(_, users)| users.clone())
                    .unwrap_or_default(),
                title_ref: MessageField::some(title),
                ..TitleRequesters::default()
            }
        };
        Ok(Requesters {
            titles: asked.titles.into_iter().map(of_title).collect(),
            ..Requesters::default()
        })
    }

    fn list_watchlist(&self, asked: &WatchlistAsked) -> Result<Watchlist, String> {
        let watchlists = granted(self.watchlists.as_ref(), "Read Watchlists")?;
        Ok(Watchlist {
            titles: watchlists.get(&asked.user_id).cloned().unwrap_or_default(),
            ..Watchlist::default()
        })
    }

    fn start_delete(&mut self, asked: DeleteAsked) -> Result<DeleteStarted, String> {
        let handler = granted(self.delete_media.as_mut(), "Delete media")?;
        let title = asked.title_ref.into_option().unwrap_or_default();
        let taken_along = handler(&title, &asked.also);
        self.deletes.push(Delete {
            title,
            also: asked.also,
        });
        Ok(DeleteStarted {
            taken_along: taken_along?,
            ..DeleteStarted::default()
        })
    }
}

/// Decodes `request`, has `answer` answer it, and encodes its response.
fn typed<Asked: Message, Answer: Message>(
    request: &[u8],
    answer: impl FnOnce(Asked) -> Result<Answer, String>,
) -> Result<Answered, Error> {
    let asked = Asked::decode_from_slice(request)?;
    Ok(answer(asked).map(|answer| answer.encode_to_vec()))
}

/// `data`, or the Kernel's refusal when the Admin didn't grant the Plugin `permission`, named as
/// the Server names it.
fn granted<T>(data: Option<T>, permission: &str) -> Result<T, String> {
    data.ok_or_else(|| format!("the Admin hasn't granted the Plugin the {permission} Permission"))
}

#[expect(
    clippy::expect_used,
    reason = "a Plugin called outside `with` is the test's bug"
)]
fn installed<R>(act: impl FnOnce(&mut Kernel) -> R) -> R {
    INSTALLED.with_borrow_mut(|kernel| {
        act(kernel
            .as_mut()
            .expect("call the Plugin inside nama_pdk::testing::with, which gives it a Kernel"))
    })
}

#[cfg(test)]
mod tests {
    use ::extism_pdk::{Error, HttpRequest};
    use buffa::MessageField;

    use super::{Kernel, with};
    use crate::{
        Library, Response, Title, TitleId, TitleKind, TitleRef, TitleRefKind, TitleWatchStates,
        User, WatchState, WatchlistTitle,
    };

    fn get(url: &str) -> Result<Response, Error> {
        crate::http_request(&HttpRequest::new(url), None::<String>, "nothing answered")
    }

    #[test]
    fn the_handlers_answer_reaches_the_plugin() {
        let mut kernel = Kernel::new().http(|_request, _body| Ok(Response::new(201, "made")));

        let answered = with(&mut kernel, || get("http://service.test/things")).unwrap();

        assert_eq!(
            (answered.status_code(), answered.body()),
            (201, &b"made"[..])
        );
    }

    #[test]
    fn a_scripted_header_reads_back_in_any_case() {
        let mut kernel = Kernel::new()
            .http(|_request, _body| Ok(Response::new(429, "").with_header("Retry-After", "120")));

        let answered = with(&mut kernel, || get("http://service.test")).unwrap();

        let read = [
            "Retry-After",
            "retry-after",
            "RETRY-AFTER",
            "ratelimit-reset",
        ]
        .map(|name| answered.header(name));
        assert_eq!(read, [Some("120"), Some("120"), Some("120"), None]);
    }

    #[test]
    fn requests_are_recorded_in_the_order_sent_with_their_bodies() {
        let mut kernel = Kernel::new().http(|_request, _body| Ok(Response::new(204, "")));

        with(&mut kernel, || {
            let post = HttpRequest::new("http://service.test/a").with_method("POST");
            crate::http_request(&post, Some("first"), "nothing answered")?;
            get("http://service.test/b")
        })
        .unwrap();

        let sent: Vec<_> = (kernel.sent().iter())
            .map(|sent| {
                let method = sent.request.method.as_deref();
                (method, sent.request.url.as_str(), sent.body.as_slice())
            })
            .collect();
        assert_eq!(
            sent,
            [
                (Some("POST"), "http://service.test/a", &b"first"[..]),
                (None, "http://service.test/b", &b""[..]),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "the Plugin sent GET http://service.test/unscripted")]
    fn an_unscripted_request_panics() {
        let _unanswered = with(&mut Kernel::new(), || get("http://service.test/unscripted"));
    }

    #[test]
    fn a_host_that_cant_be_reached_fails_with_the_plugins_words() {
        let mut kernel = Kernel::new().http(|_request, _body| Err(Error::msg("refused")));

        let failed = with(&mut kernel, || get("http://service.test")).unwrap_err();

        assert_eq!(failed.to_string(), "nothing answered");
    }

    #[test]
    fn a_granted_store_keeps_each_value_by_key_and_user() {
        let mut kernel = Kernel::new().store();

        let read = with(&mut kernel, || {
            crate::kernel::store_put("token", None, b"plugin's")?;
            crate::kernel::store_put("token", Some(7), b"user's")?;
            crate::kernel::store_put("gone", None, b"soon")?;
            crate::kernel::store_delete("gone", None)?;
            ["token", "gone"]
                .into_iter()
                .flat_map(|key| [None, Some(7)].map(|user| crate::kernel::store_get(key, user)))
                .collect::<Result<Vec<_>, Error>>()
        })
        .unwrap();

        let kept = |value: &[u8]| Some(value.to_vec());
        assert_eq!(read, [kept(b"plugin's"), kept(b"user's"), None, None]);
    }

    #[test]
    fn a_store_not_granted_is_refused() {
        let refused = with(&mut Kernel::new(), || {
            crate::kernel::store_get("token", None).unwrap_err()
        });

        assert_eq!(
            refused.to_string(),
            "the Admin hasn't granted the Plugin the Store Permission"
        );
    }

    #[test]
    fn granted_libraries_answer() {
        let movies = Library {
            id: 1,
            name: "Movies".to_owned(),
            ..Library::default()
        };
        let mut kernel = Kernel::new().libraries([movies.clone()]);

        let listed = with(&mut kernel, crate::kernel::libraries).unwrap();

        assert_eq!(listed, [movies]);
    }

    fn movie(library_id: i64, id: &str) -> TitleRef {
        TitleRef {
            library_id,
            kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
            id: MessageField::some(TitleId {
                namespace: "tmdb".to_owned(),
                value: id.to_owned(),
                ..TitleId::default()
            }),
            ..TitleRef::default()
        }
    }

    fn titled(title_ref: TitleRef) -> Title {
        Title {
            title_ref: MessageField::some(title_ref),
            ..Title::default()
        }
    }

    fn titles_of_library_1(count: usize) -> Vec<Title> {
        (0..count)
            .map(|id| titled(movie(1, &id.to_string())))
            .collect()
    }

    fn ids(titles: &[Title]) -> Vec<&str> {
        (titles.iter())
            .map(|title| title.title_ref.id.value.as_str())
            .collect()
    }

    #[test]
    fn titles_page_by_the_size_asked_until_the_last_page_has_no_token() {
        let mut kernel = Kernel::new().titles(1, titles_of_library_1(3));

        let (first, last) = with(&mut kernel, || {
            let first = crate::kernel::titles_page(1, 2, "")?;
            let last = crate::kernel::titles_page(1, 2, &first.next_page_token)?;
            Ok::<_, Error>((first, last))
        })
        .unwrap();

        assert_eq!(ids(&first.titles), ["0", "1"]);
        assert_eq!(
            (ids(&last.titles), last.next_page_token.as_str()),
            (vec!["2"], "")
        );
    }

    #[test]
    fn a_page_size_over_1000_is_held_to_1000() {
        let mut kernel = Kernel::new().titles(1, titles_of_library_1(1001));

        let page = with(&mut kernel, || crate::kernel::titles_page(1, 5000, "")).unwrap();

        assert_eq!(page.titles.len(), 1000);
        assert_ne!(page.next_page_token, "");
    }

    #[test]
    fn a_page_size_of_0_asks_for_every_title_at_once() {
        let mut kernel = Kernel::new().titles(1, titles_of_library_1(1001));

        let page = with(&mut kernel, || crate::kernel::titles_page(1, 0, "")).unwrap();

        assert_eq!(
            (page.titles.len(), page.next_page_token.as_str()),
            (1001, "")
        );
    }

    #[test]
    fn a_page_token_the_kernel_didnt_hand_out_is_refused() {
        let mut kernel = Kernel::new().titles(1, titles_of_library_1(3));

        let refused = with(&mut kernel, || {
            crate::kernel::titles_page(1, 2, "forged").unwrap_err()
        });

        assert_eq!(
            refused.to_string(),
            r#"the page token "forged" isn't one the Kernel handed out"#
        );
    }

    #[test]
    fn granted_titles_grant_read_libraries_and_a_library_without_any_has_none() {
        let mut kernel = Kernel::new().titles(1, titles_of_library_1(2));

        let read = with(&mut kernel, || {
            Ok::<_, Error>((
                crate::kernel::libraries()?,
                crate::kernel::titles(1)?,
                crate::kernel::titles(2)?,
            ))
        })
        .unwrap();

        assert_eq!(read, (vec![], titles_of_library_1(2), vec![]));
    }

    fn watched_by(user_id: i64) -> WatchState {
        WatchState {
            user_id,
            played: true,
            ..WatchState::default()
        }
    }

    #[test]
    fn watch_states_answer_each_title_asked_and_only_the_named_users_when_one_is() {
        let mut kernel = Kernel::new().watch_states(&movie(1, "1"), [watched_by(7), watched_by(8)]);

        let (everyones, eights) = with(&mut kernel, || {
            let everyones = crate::kernel::watch_states(&[movie(1, "1"), movie(1, "2")], None)?;
            let eights = crate::kernel::watch_states(&[movie(1, "1")], Some(8))?;
            Ok::<_, Error>((everyones, eights))
        })
        .unwrap();

        let users = |answered: Vec<TitleWatchStates>| -> Vec<(TitleRef, Vec<i64>)> {
            (answered.into_iter())
                .map(|title| {
                    let users = title.states.iter().map(|state| state.user_id).collect();
                    (title.title_ref.into_option().unwrap_or_default(), users)
                })
                .collect()
        };
        assert_eq!(
            users(everyones),
            [(movie(1, "1"), vec![7, 8]), (movie(1, "2"), vec![])]
        );
        assert_eq!(users(eights), [(movie(1, "1"), vec![8])]);
    }

    #[test]
    fn granted_users_answer() {
        let user = User {
            id: 7,
            display_name: "Ana".to_owned(),
            ..User::default()
        };
        let mut kernel = Kernel::new().users([user.clone()]);

        let listed = with(&mut kernel, crate::kernel::users).unwrap();

        assert_eq!(listed, [user]);
    }

    #[test]
    fn requesters_answer_each_title_and_whether_anything_requested_it() {
        let mut kernel = Kernel::new()
            .requesters(movie(1, "1"), [7])
            .requesters(movie(1, "2"), []);

        let asked = [movie(1, "1"), movie(1, "2"), movie(1, "3")];
        let answered = with(&mut kernel, || crate::kernel::requesters(&asked)).unwrap();

        let read: Vec<_> = (answered.iter())
            .map(|title| (title.requested, title.user_ids.as_slice()))
            .collect();
        assert_eq!(read, [(true, &[7][..]), (true, &[]), (false, &[])]);
    }

    #[test]
    fn a_users_watchlist_answers_and_another_users_is_empty() {
        let wanted = WatchlistTitle {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            ..WatchlistTitle::default()
        };
        let mut kernel = Kernel::new().watchlist(7, [wanted.clone()]);

        let read = with(&mut kernel, || {
            Ok::<_, Error>((crate::kernel::watchlist(7)?, crate::kernel::watchlist(8)?))
        })
        .unwrap();

        assert_eq!(read, (vec![wanted], vec![]));
    }

    #[test]
    fn deletes_are_recorded_in_order_and_the_handler_answers_what_goes_along() {
        let mut kernel = Kernel::new().delete_media(|_title, also| Ok(also.to_vec()));

        let taken_along = with(&mut kernel, || {
            crate::kernel::delete_media(movie(1, "1"), &[])?;
            crate::kernel::delete_media(movie(1, "2"), &[movie(1, "3")])
        })
        .unwrap();

        assert_eq!(taken_along, [movie(1, "3")]);
        let asked: Vec<_> = (kernel.deletes().iter())
            .map(|delete| (delete.title.clone(), delete.also.clone()))
            .collect();
        assert_eq!(
            asked,
            [
                (movie(1, "1"), vec![]),
                (movie(1, "2"), vec![movie(1, "3")]),
            ]
        );
    }

    #[test]
    fn a_refused_delete_reaches_the_plugin_in_the_handlers_words_and_is_recorded() {
        let mut kernel =
            Kernel::new().delete_media(|_title, _also| Err("the files are read-only".to_owned()));

        let refused = with(&mut kernel, || {
            crate::kernel::delete_media(movie(1, "1"), &[]).unwrap_err()
        });

        assert_eq!(refused.to_string(), "the files are read-only");
        assert_eq!(kernel.deletes().len(), 1);
    }

    #[test]
    fn each_function_not_granted_is_refused_naming_its_permission() {
        let refused = with(&mut Kernel::new(), || {
            [
                crate::kernel::libraries().map(drop),
                crate::kernel::titles(1).map(drop),
                crate::kernel::watch_states(&[], None).map(drop),
                crate::kernel::users().map(drop),
                crate::kernel::requesters(&[]).map(drop),
                crate::kernel::watchlist(7).map(drop),
                crate::kernel::delete_media(movie(1, "1"), &[]).map(drop),
            ]
            .map(|answered| answered.unwrap_err().to_string())
        });

        let not_granted =
            |permission| format!("the Admin hasn't granted the Plugin the {permission} Permission");
        assert_eq!(
            refused,
            [
                "Read Libraries",
                "Read Libraries",
                "Read Watch States",
                "Read Users",
                "Read Requests",
                "Read Watchlists",
                "Delete media",
            ]
            .map(not_granted)
        );
    }

    #[test]
    fn settings_read_back_and_an_unset_one_is_none() {
        let mut kernel = Kernel::new().setting("url", "http://service.test");

        let read = with(&mut kernel, || {
            (
                crate::setting("url").unwrap(),
                crate::setting("api_key").unwrap(),
            )
        });

        assert_eq!(read, (Some("http://service.test".to_owned()), None));
    }
}
