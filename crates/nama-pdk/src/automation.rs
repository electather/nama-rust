//! The Automation lane, where the Kernel ticks an Automation Plugin, and the Kernel's functions,
//! which any Plugin may call.

use extism_pdk::Error;
pub use nama_plugin_proto::nama::plugin::v1::{
    Library, LibraryKind, Permission, Tick, Title, TitleRef, TitleRefKind, TitleRequesters,
    TitleWatchStates, Titles, User, WatchState, WatchlistTitle,
};

/// An Automation Plugin: the Kernel calls [`Automation::tick`] every `tick_every_seconds` its
/// Manifest sets, and in it the Plugin may call the Kernel's functions in [`kernel`]. Its
/// Manifest's `points` name [`services::AUTOMATION`](crate::services::AUTOMATION) and its
/// `permissions` the Permissions those functions need, and [`automation!`](crate::automation!)
/// exports it:
///
/// ```
/// use nama_pdk::{Automation, Tick, extism_pdk::Error};
///
/// struct Cleanup;
///
/// impl Automation for Cleanup {
///     fn tick(_tick: Tick) -> Result<(), Error> {
///         for library in nama_pdk::kernel::libraries()? {
///             let _titles = nama_pdk::kernel::titles(library.id)?;
///         }
///         Ok(())
///     }
/// }
///
/// nama_pdk::automation!(Cleanup);
/// ```
pub trait Automation {
    /// Does the Plugin's scheduled work. `tick.now_ms` is the Kernel's clock.
    ///
    /// # Errors
    ///
    /// When the work failed, such as a Kernel function refused: the Kernel logs the error's
    /// message and ticks again on schedule.
    fn tick(tick: Tick) -> Result<(), Error>;
}

/// Exports an [`Automation`] as the Plugin's `tick`, taking the `nama.plugin.v1.Tick` the Kernel
/// sends it and returning a `nama.plugin.v1.Ticked`.
#[macro_export]
macro_rules! automation {
    ($automation:ty) => {
        const _: () = {
            // `plugin_fn` expands to paths starting with `extism_pdk`.
            use $crate::{
                Automation, Buffa, FnResult, Tick, extism_pdk,
                nama_plugin_proto::nama::plugin::v1::Ticked, plugin_fn,
            };

            #[plugin_fn]
            pub fn tick(Buffa(tick): Buffa<Tick>) -> FnResult<Buffa<Ticked>> {
                <$automation as Automation>::tick(tick)?;
                Ok(Buffa(Ticked::default()))
            }
        };
    };
}

/// The Kernel's functions, which any Plugin, whatever its Manifest's `points`, may call during any
/// of its calls (ADR 0071). Each needs a Plugin Permission the Manifest requests in `permissions`
/// and the Admin granted, and fails with the Kernel's plain words when it wasn't. Off wasm32, as
/// in a Plugin's unit tests, each fails, since there is no Kernel to call.
pub mod kernel {
    use buffa::Message;
    use extism_pdk::Error;
    use nama_plugin_proto::nama::plugin::v1::{
        DeleteAsked, DeleteStarted, KernelAnswer, Libraries, LibrariesAsked, Library, Requesters,
        RequestersAsked, StoreEntry, StoreKey, StoreValue, Stored, Title, TitleRef,
        TitleRequesters, TitleWatchStates, Titles, TitlesAsked, User, Users, UsersAsked,
        WatchStates, WatchStatesAsked, Watchlist, WatchlistAsked, WatchlistTitle, kernel_answer,
    };

    /// The value the Plugin's Store keeps under `key`, for `user` or, with `None`, for the Plugin
    /// alone; `None` if there is none. Needs `PERMISSION_STORE`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn store_get(key: &str, user: Option<i64>) -> Result<Option<Vec<u8>>, Error> {
        let asked = StoreKey {
            key: key.to_owned(),
            user_id: user,
            ..StoreKey::default()
        };
        Ok(call::<StoreValue>("store_get", &asked)?.value)
    }

    /// Keeps `value` under `key` in the Plugin's Store, for `user` or, with `None`, for the Plugin
    /// alone, replacing what was there. Needs `PERMISSION_STORE`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, such as a key over 256 bytes, a value over 64 KiB or a new key
    /// once the Plugin keeps 10 000 entries, or couldn't be called.
    pub fn store_put(key: &str, user: Option<i64>, value: &[u8]) -> Result<(), Error> {
        let entry = StoreEntry {
            key: key.to_owned(),
            user_id: user,
            value: value.to_vec(),
            ..StoreEntry::default()
        };
        call::<Stored>("store_put", &entry).map(drop)
    }

    /// Deletes what the Plugin's Store keeps under `key`, for `user` or, with `None`, for the
    /// Plugin alone. Needs `PERMISSION_STORE`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn store_delete(key: &str, user: Option<i64>) -> Result<(), Error> {
        let asked = StoreKey {
            key: key.to_owned(),
            user_id: user,
            ..StoreKey::default()
        };
        call::<Stored>("store_delete", &asked).map(drop)
    }

    /// Every Library. Needs `PERMISSION_READ_LIBRARIES`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn libraries() -> Result<Vec<Library>, Error> {
        Ok(call::<Libraries>("list_libraries", &LibrariesAsked::default())?.libraries)
    }

    /// The Movies of the Library `library_id`, or its Shows' Episodes, by kind and then id,
    /// asked for a page at a time. Needs `PERMISSION_READ_LIBRARIES`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn titles(library_id: i64) -> Result<Vec<Title>, Error> {
        let mut titles = Vec::new();
        let mut token = String::new();
        loop {
            let page = titles_page(library_id, MAX_PAGE_SIZE, &token)?;
            titles.extend(page.titles);
            if page.next_page_token.is_empty() {
                return Ok(titles);
            }
            token = page.next_page_token;
        }
    }

    /// The most titles one page of [`titles_page`] holds.
    const MAX_PAGE_SIZE: u32 = 1000;

    /// A page of up to `page_size` of the Library `library_id`'s titles, by kind and then id,
    /// from `page_token`: empty for the first page, or the `next_page_token` of the page before,
    /// which is empty on the last. A `page_size` of 0 asks for every title at once, and the
    /// Kernel holds one over 1000 to 1000. Needs `PERMISSION_READ_LIBRARIES`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, such as for a `page_token` it didn't hand out, or couldn't be
    /// called.
    pub fn titles_page(library_id: i64, page_size: u32, page_token: &str) -> Result<Titles, Error> {
        let asked = TitlesAsked {
            library_id,
            page_size,
            page_token: page_token.to_owned(),
            ..TitlesAsked::default()
        };
        call::<Titles>("list_titles", &asked)
    }

    /// Each of `titles` with every User's Watch State of it, or with `Some(user)` only that
    /// User's. Needs `PERMISSION_READ_WATCH_STATES`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn watch_states(
        titles: &[TitleRef],
        user: Option<i64>,
    ) -> Result<Vec<TitleWatchStates>, Error> {
        let asked = WatchStatesAsked {
            titles: titles.to_vec(),
            user_id: user,
            ..WatchStatesAsked::default()
        };
        Ok(call::<WatchStates>("list_watch_states", &asked)?.titles)
    }

    /// Every User, with their id and display name only. Every User sees every Library, so these
    /// are every User who can watch a title, including those without any Watch State. Needs
    /// `PERMISSION_READ_USERS`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn users() -> Result<Vec<User>, Error> {
        Ok(call::<Users>("list_users", &UsersAsked::default())?.users)
    }

    /// Each of `titles` with the ids of the Users who Requested it, and whether anything did, so
    /// a title whose Requesters were all deleted is told from one nobody Requested. Needs
    /// `PERMISSION_READ_REQUESTS`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn requesters(titles: &[TitleRef]) -> Result<Vec<TitleRequesters>, Error> {
        let asked = RequestersAsked {
            titles: titles.to_vec(),
            ..RequestersAsked::default()
        };
        Ok(call::<Requesters>("list_requesters", &asked)?.titles)
    }

    /// The User `user`'s Watchlist, each title by the id it was added under, whether or not a
    /// Library has it. Needs `PERMISSION_READ_WATCHLISTS`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn watchlist(user: i64) -> Result<Vec<WatchlistTitle>, Error> {
        let asked = WatchlistAsked {
            user_id: user,
            ..WatchlistAsked::default()
        };
        Ok(call::<Watchlist>("list_watchlist", &asked)?.titles)
    }

    /// Deletes the media files of `title`, as an Admin's delete does. It returns once the Kernel
    /// checked the delete, which goes on in the background, with the other titles that go with
    /// it: Episodes a file of `title` holds too, each of which `also` must name. Needs
    /// `PERMISSION_DELETE_MEDIA`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, such as for a title it doesn't have or a file holding an Episode
    /// `also` doesn't name, which the refusal names, or couldn't be called.
    pub fn delete_media(title: TitleRef, also: &[TitleRef]) -> Result<Vec<TitleRef>, Error> {
        let asked = DeleteAsked {
            title_ref: title.into(),
            also: also.to_vec(),
            ..DeleteAsked::default()
        };
        Ok(call::<DeleteStarted>("delete_media", &asked)?.taken_along)
    }

    /// Calls the Kernel function `name` with `request` and decodes its answer.
    fn call<T: Message>(name: &str, request: &impl Message) -> Result<T, Error> {
        answer(name, &raw(name, &request.try_encode_to_vec()?)?)
    }

    /// The response a `KernelAnswer` encoded as `bytes` holds, or the Kernel's words as the error
    /// when it refused.
    pub(super) fn answer<T: Message>(name: &str, bytes: &[u8]) -> Result<T, Error> {
        match KernelAnswer::decode_from_slice(bytes)?.result {
            Some(kernel_answer::Result::Answer(answer)) => Ok(T::decode_from_slice(&answer)?),
            Some(kernel_answer::Result::Refused(words)) => Err(Error::msg(words)),
            None => Err(Error::msg(format!("the Kernel answered nothing to {name}"))),
        }
    }

    /// Calls the Wasm import `name` in the `nama` namespace with `request`, returning the
    /// `KernelAnswer` it answered, encoded.
    #[cfg(target_arch = "wasm32")]
    fn raw(name: &str, request: &[u8]) -> Result<Vec<u8>, Error> {
        use extism_pdk::{ManagedMemory, Memory};

        // Each takes and returns the offset of an Extism memory block, as Extism's host functions
        // do. The Kernel only reads the request and writes a fresh answer, whatever the offset.
        #[expect(unsafe_code, reason = "the Kernel's functions are Wasm imports")]
        #[link(wasm_import_module = "nama")]
        unsafe extern "C" {
            safe fn store_get(request: u64) -> u64;
            safe fn store_put(request: u64) -> u64;
            safe fn store_delete(request: u64) -> u64;
            safe fn list_libraries(request: u64) -> u64;
            safe fn list_titles(request: u64) -> u64;
            safe fn list_watch_states(request: u64) -> u64;
            safe fn list_users(request: u64) -> u64;
            safe fn list_requesters(request: u64) -> u64;
            safe fn list_watchlist(request: u64) -> u64;
            safe fn delete_media(request: u64) -> u64;
        }

        let import: extern "C" fn(u64) -> u64 = match name {
            "store_get" => store_get,
            "store_put" => store_put,
            "store_delete" => store_delete,
            "list_libraries" => list_libraries,
            "list_titles" => list_titles,
            "list_watch_states" => list_watch_states,
            "list_users" => list_users,
            "list_requesters" => list_requesters,
            "list_watchlist" => list_watchlist,
            "delete_media" => delete_media,
            _ => return Err(Error::msg(format!("the Kernel has no function {name}"))),
        };
        // Freed when dropped, once the Kernel has read it.
        let request = ManagedMemory::from(Memory::from_bytes(request)?);
        let answer = Memory::find(import(request.offset()))
            .ok_or_else(|| Error::msg(format!("the Kernel answered nothing to {name}")))?;
        let bytes = answer.to_vec();
        answer.free();
        Ok(bytes)
    }

    /// Off wasm32 there is no Kernel, so every function fails, and a Plugin's native unit tests
    /// and doctests still link.
    #[cfg(not(target_arch = "wasm32"))]
    fn raw(_name: &str, _request: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::msg(
            "the Kernel's functions exist only inside the Kernel",
        ))
    }
}

#[cfg(test)]
mod tests {
    use buffa::Message;
    use nama_plugin_proto::nama::plugin::v1::{KernelAnswer, StoreValue, kernel_answer};

    use super::{Automation, Tick, kernel};
    use crate::extism_pdk::Error;

    struct Ticker;

    impl Automation for Ticker {
        fn tick(_tick: Tick) -> Result<(), Error> {
            Ok(())
        }
    }

    // Links only if `automation!` expands to a well-formed `tick` export.
    crate::automation!(Ticker);

    fn encoded(result: kernel_answer::Result) -> Vec<u8> {
        KernelAnswer {
            result: Some(result),
            ..KernelAnswer::default()
        }
        .encode_to_vec()
    }

    #[test]
    fn an_answer_decodes_to_the_response() {
        let value = StoreValue {
            value: Some(b"3".to_vec()),
            ..StoreValue::default()
        };
        let bytes = encoded(kernel_answer::Result::Answer(value.encode_to_vec()));

        let answered: StoreValue = kernel::answer("store_get", &bytes).expect("an answer");

        assert_eq!(answered.value.as_deref(), Some(&b"3"[..]));
    }

    #[test]
    fn a_refusal_is_the_kernels_words() {
        let words = "the Admin didn't grant this Plugin the Store Permission";
        let bytes = encoded(kernel_answer::Result::Refused(words.to_owned()));

        let error = kernel::answer::<StoreValue>("store_get", &bytes).expect_err("a refusal");

        assert_eq!(error.to_string(), words);
    }

    #[test]
    fn off_wasm32_every_kernel_function_fails() {
        let error = kernel::store_get("counter", None).expect_err("no Kernel");

        assert_eq!(
            error.to_string(),
            "the Kernel's functions exist only inside the Kernel"
        );
    }
}
