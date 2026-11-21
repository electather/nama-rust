//! The Kernel's functions, which any Plugin may call.

pub use nama_plugin_proto::nama::plugin::v1::{
    Library, LibraryKind, Permission, SharingTitle, Title, TitleRef, TitleRefKind, TitleRequesters,
    TitleRole, TitleWatchStates, Titles, User, WatchState, WatchlistTitle,
};

/// The Kernel's functions, which any Plugin, whatever its Manifest's `points`, may call during any
/// of its calls (ADR 0071). Each needs a Plugin Permission the Manifest requests in `permissions`
/// and the Admin granted, and fails with the Kernel's plain words when it wasn't. Off wasm32, as
/// in a Plugin's unit tests, the [`testing::Kernel`](crate::testing::Kernel) a test installs with
/// [`testing::with`](crate::testing::with) answers every one of them; called outside `with`, each
/// panics.
pub mod kernel {
    use buffa::Message;
    use extism_pdk::Error;
    use nama_plugin_proto::nama::plugin::v1::{
        BackUpAsked, BackedUp, DeleteAsked, DeleteStarted, ExpiredSignInsAsked,
        ExpiredSignInsDeleted, ForgetGonePluginsAsked, ForgetHomeRowsAsked, GonePluginsForgotten,
        HomeRowsForgotten, KernelAnswer, Libraries, LibrariesAsked, Library, MetadataRefreshed,
        RefreshMetadataAsked, Requesters, RequestersAsked, SharingTitle, SharingTitles, StoreEntry,
        StoreKey, StoreValue, Stored, Title, TitleId, TitleKind, TitleRef, TitleRequesters,
        TitleWatchStates, Titles, TitlesAsked, TitlesSharingAsked, User, Users, UsersAsked,
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
    pub(crate) const MAX_PAGE_SIZE: u32 = 1000;

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

    /// Deletes each User's own Home rows and Featured suggestions, for each User who hasn't opened
    /// home in at least `days` days, and answers how many Users had some. Needs
    /// `PERMISSION_FORGET_HOME_ROWS`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, such as for `days` of 0, or couldn't be called.
    pub fn forget_home_rows(days: u32) -> Result<u32, Error> {
        let asked = ForgetHomeRowsAsked {
            days,
            ..ForgetHomeRowsAsked::default()
        };
        Ok(call::<HomeRowsForgotten>("forget_home_rows", &asked)?.users)
    }

    /// Deletes every Device whose session expired and every Invite past its expiry, and answers
    /// how many of each, as `(devices, invites)`. Needs `PERMISSION_DELETE_EXPIRED_SIGN_INS`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn delete_expired_sign_ins() -> Result<(u32, u32), Error> {
        let deleted = call::<ExpiredSignInsDeleted>(
            "delete_expired_sign_ins",
            &ExpiredSignInsAsked::default(),
        )?;
        Ok((deleted.devices, deleted.invites))
    }

    /// Deletes what each Plugin the Server hasn't had for at least `days` days left behind, such
    /// as its Store entries and settings, and answers their ids. A Plugin the Server still has,
    /// enabled or not, is never one of them. Needs `PERMISSION_FORGET_GONE_PLUGINS`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, such as for `days` of 0, or couldn't be called.
    pub fn forget_gone_plugins(days: u32) -> Result<Vec<String>, Error> {
        let asked = ForgetGonePluginsAsked {
            days,
            ..ForgetGonePluginsAsked::default()
        };
        Ok(call::<GonePluginsForgotten>("forget_gone_plugins", &asked)?.plugin_ids)
    }

    /// Up to `limit` (at most 1000, 0 for 1000) Movies or Shows of `kind` from every Library, each
    /// once, sharing at least one of `people` or `genres`: those sharing the most Persons first,
    /// then the most genres, each with its ids, genres and every Role. Needs
    /// `PERMISSION_READ_LIBRARIES`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, such as for a kind that is neither Movie nor Show, or couldn't be
    /// called.
    pub fn titles_sharing(
        kind: TitleKind,
        people: &[TitleId],
        genres: &[String],
        limit: u32,
    ) -> Result<Vec<SharingTitle>, Error> {
        let asked = TitlesSharingAsked {
            kind: kind.into(),
            people: people.to_vec(),
            genres: genres.to_vec(),
            limit,
            ..TitlesSharingAsked::default()
        };
        Ok(call::<SharingTitles>("list_titles_sharing", &asked)?.titles)
    }

    /// Fetches every Matched title of every Library again from its title source, by the ids its
    /// Match has, whatever its metadata version, and stores what it got, as
    /// `(titles, unavailable_plugin_ids)`: how many titles it stored, and the id of each Plugin
    /// that couldn't answer, whose titles keep what they had. It never searches or changes a
    /// Match. Needs `PERMISSION_REFRESH_METADATA`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused or couldn't be called.
    pub fn refresh_metadata() -> Result<(u64, Vec<String>), Error> {
        let refreshed =
            call::<MetadataRefreshed>("refresh_metadata", &RefreshMetadataAsked::default())?;
        Ok((refreshed.titles, refreshed.unavailable_plugin_ids))
    }

    /// Writes a Backup of the Server (ADR 0068) named `nama-<UTC time>.tar` into the Backup
    /// folder the Admin set, as `(name, size_bytes)`: its file name and size. It then deletes the
    /// oldest so named beyond the count the Admin keeps; a file named otherwise is never deleted,
    /// and a Backup that fails deletes nothing. Needs `PERMISSION_BACK_UP`.
    ///
    /// # Errors
    ///
    /// When the Kernel refused, couldn't write the Backup, or couldn't be called.
    pub fn back_up() -> Result<(String, u64), Error> {
        let backed_up = call::<BackedUp>("back_up", &BackUpAsked::default())?;
        Ok((backed_up.name, backed_up.size_bytes))
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
            safe fn forget_home_rows(request: u64) -> u64;
            safe fn delete_expired_sign_ins(request: u64) -> u64;
            safe fn forget_gone_plugins(request: u64) -> u64;
            safe fn list_titles_sharing(request: u64) -> u64;
            safe fn refresh_metadata(request: u64) -> u64;
            safe fn back_up(request: u64) -> u64;
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
            "forget_home_rows" => forget_home_rows,
            "delete_expired_sign_ins" => delete_expired_sign_ins,
            "forget_gone_plugins" => forget_gone_plugins,
            "list_titles_sharing" => list_titles_sharing,
            "refresh_metadata" => refresh_metadata,
            "back_up" => back_up,
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

    /// Off wasm32 the installed [`testing::Kernel`](crate::testing::Kernel) answers.
    #[cfg(not(target_arch = "wasm32"))]
    fn raw(name: &str, request: &[u8]) -> Result<Vec<u8>, Error> {
        crate::testing::kernel_function(name, request)
    }
}

#[cfg(test)]
mod tests {
    use buffa::Message;
    use nama_plugin_proto::nama::plugin::v1::{KernelAnswer, StoreValue, kernel_answer};

    use super::kernel;

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
}
