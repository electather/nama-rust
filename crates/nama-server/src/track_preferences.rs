//! Track Preferences: what a User wants a kind of Track to start as when they play something (ADR
//! 0035): the Subtitle and Audio Preferences a User's pick keeps for a Show or Movie, and the
//! User's own, set in Settings.
//!
//! [`TrackPreferences`] alone reads and writes `track_preferences` (`AGENTS.md`, "Data"). It keeps
//! a pick's against a Show's or Movie's external id, as Watch State is kept (ADR 0013), so an
//! Unmatched Item keeps none, and the User's own at the `user` scope.

use std::sync::Arc;

use isolang::Language;

use crate::{
    contents::{Contents, MatchedTo},
    db::Database,
    tracks::Track,
};

// The `kind` of a Preference, part of its key.
const SUBTITLE: &str = "subtitle";
const AUDIO: &str = "audio";

/// Reads and keeps each User's Track Preferences.
#[derive(Debug, Clone)]
pub(crate) struct TrackPreferences {
    db: Arc<Database>,
    contents: Contents,
}

/// What a User wants subtitles to start as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubtitlePreference {
    Off,
    /// A Track like the one picked.
    Like(Description),
}

/// What describes the Track a User picked, all a Preference keeps of it: not its place, since each
/// Episode is its own file, nor its format, since a Device decides whether it draws it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Description {
    /// As the file tagged it, usually ISO 639-2, such as `eng`.
    pub(crate) language: Option<String>,
    pub(crate) forced: bool,
    pub(crate) sdh: bool,
    pub(crate) title: Option<String>,
}

impl Description {
    pub(crate) fn of(track: &Track) -> Self {
        Self {
            language: track.language.clone(),
            forced: track.forced,
            sdh: track.sdh,
            title: track.title.clone(),
        }
    }
}

/// The Preference a plan starts with, a [`SubtitlePreference`] or the [`Description`] of an audio
/// Track, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preferred<P> {
    pub(crate) preference: P,
    /// The User's own, set without a title, rather than the Show's or Movie's.
    pub(crate) users_own: bool,
    /// Whether it wants full subtitles only over audio in another language, which only the User's
    /// own Subtitle Preference can (ADR 0035).
    pub(crate) foreign_audio_only: bool,
}

/// What a Preference is kept for: a Show or Movie, by its scope and external id, or everything the
/// User plays. The key after the User.
#[derive(Debug)]
pub(crate) struct KeptFor {
    scope: &'static str,
    title_id: String,
}

impl KeptFor {
    /// Everything the User plays: their own Preference, set in Settings.
    pub(crate) const USERS_OWN: Self = Self {
        scope: USER,
        title_id: String::new(),
    };
}

// The scope of a User's own Preference, kept with an empty `title_id`.
const USER: &str = "user";

/// A `track_preferences` row: what describes the Track it wants, `None` for off, and whose it is.
type Row = Preferred<Option<Description>>;

/// Why what a Version is of couldn't be found.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("there is no such Version")]
    NoSuchVersion,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl TrackPreferences {
    pub(crate) fn new(db: Arc<Database>) -> Self {
        Self {
            contents: Contents::new(Arc::clone(&db)),
            db,
        }
    }

    /// Keeps `preference` as the User `user`'s Subtitle Preference for `kept_for`, in place of any
    /// they had.
    pub(crate) async fn keep_subtitle(
        &self,
        user: i64,
        kept_for: KeptFor,
        preference: SubtitlePreference,
    ) -> Result<(), sqlx::Error> {
        self.keep(user, kept_for, SUBTITLE, like(preference), false)
            .await
    }

    /// Keeps `preference` as the User `user`'s own Subtitle Preference, wanting it only over audio
    /// in another language if `foreign_audio_only`, in place of any they had.
    pub(crate) async fn keep_users_subtitle(
        &self,
        user: i64,
        preference: SubtitlePreference,
        foreign_audio_only: bool,
    ) -> Result<(), sqlx::Error> {
        let like = like(preference);
        (self.keep(user, KeptFor::USERS_OWN, SUBTITLE, like, foreign_audio_only)).await
    }

    /// Keeps a Track like `picked` as the User `user`'s Audio Preference for `kept_for`, in place
    /// of any they had. Only its language and title are kept: forced and SDH describe subtitles.
    pub(crate) async fn keep_audio(
        &self,
        user: i64,
        kept_for: KeptFor,
        picked: Description,
    ) -> Result<(), sqlx::Error> {
        let description = Description {
            language: picked.language,
            title: picked.title,
            ..Description::default()
        };
        (self.keep(user, kept_for, AUDIO, Some(description), false)).await
    }

    /// The Subtitle Preference a plan of the Version `version` starts with for the User `user`:
    /// the one kept for the Show or Movie it is of, which wins even when no Track matches it, or
    /// else their own. `None` if they have neither.
    pub(crate) async fn subtitle(
        &self,
        user: i64,
        version: i64,
    ) -> Result<Option<Preferred<SubtitlePreference>>, sqlx::Error> {
        let row = self.kept(user, version, SUBTITLE).await?;
        Ok(row.map(preferred_subtitle))
    }

    /// The User `user`'s own Subtitle Preference, set in Settings, `None` if they never set one.
    pub(crate) async fn users_subtitle(
        &self,
        user: i64,
    ) -> Result<Option<Preferred<SubtitlePreference>>, sqlx::Error> {
        let row = self.row(user, KeptFor::USERS_OWN, SUBTITLE).await?;
        Ok(row.map(preferred_subtitle))
    }

    /// Keeps audio in `language` as the User `user`'s own Audio Preference, or `None` for the
    /// file's default, which keeps none since audio is never off.
    pub(crate) async fn keep_users_audio(
        &self,
        user: i64,
        language: Option<String>,
    ) -> Result<(), sqlx::Error> {
        let Some(language) = language else {
            return self.forget_users_audio(user).await;
        };
        let description = Description {
            language: Some(language),
            ..Description::default()
        };
        self.keep_audio(user, KeptFor::USERS_OWN, description).await
    }

    /// The language of the User `user`'s own Audio Preference, set in Settings, `None` if they
    /// never set one.
    pub(crate) async fn users_audio(&self, user: i64) -> Result<Option<String>, sqlx::Error> {
        let row = self.row(user, KeptFor::USERS_OWN, AUDIO).await?;
        Ok(row.and_then(|row| row.preference?.language))
    }

    /// What describes the audio Track a plan of the Version `version` starts with for the User
    /// `user`: the one kept for the Show or Movie it is of, which wins even when no Track matches
    /// it, or else their own. `None` if they have neither.
    pub(crate) async fn audio(
        &self,
        user: i64,
        version: i64,
    ) -> Result<Option<Preferred<Description>>, sqlx::Error> {
        let row = self.kept(user, version, AUDIO).await?;
        // Audio is never off.
        Ok(row.map(|row| Preferred {
            preference: row.preference.unwrap_or_default(),
            users_own: row.users_own,
            foreign_audio_only: false,
        }))
    }

    /// Removes the User `user`'s own Audio Preference, if they have one.
    async fn forget_users_audio(&self, user: i64) -> Result<(), sqlx::Error> {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "DELETE FROM track_preferences
                         WHERE user_id = ? AND scope = ? AND kind = ?",
                        user,
                        USER,
                        AUDIO,
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok::<_, sqlx::Error>(())
                })
            })
            .await
    }

    /// Keeps a Track like `like`, or `None` for off, as the User `user`'s `kind` Preference for
    /// `kept_for`, wanting it only over audio in another language if `foreign_audio_only`.
    #[expect(
        clippy::too_many_arguments,
        reason = "the row's key and columns, each set by a different public `keep_*`"
    )]
    async fn keep(
        &self,
        user: i64,
        KeptFor { scope, title_id }: KeptFor,
        kind: &'static str,
        like: Option<Description>,
        foreign_audio_only: bool,
    ) -> Result<(), sqlx::Error> {
        let off = like.is_none();
        let description = like.unwrap_or_default();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO track_preferences
                             (user_id, scope, title_id, kind, off, language, forced, sdh, title,
                              foreign_audio_only)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT (user_id, scope, title_id, kind) DO UPDATE
                         SET off = excluded.off, language = excluded.language,
                             forced = excluded.forced, sdh = excluded.sdh, title = excluded.title,
                             foreign_audio_only = excluded.foreign_audio_only",
                        user,
                        scope,
                        title_id,
                        kind,
                        off,
                        description.language,
                        description.forced,
                        description.sdh,
                        description.title,
                        foreign_audio_only,
                    )
                    .execute(&mut **tx)
                    .await?;
                    Ok::<_, sqlx::Error>(())
                })
            })
            .await
    }

    /// The User `user`'s `kind` Preference for the Show or Movie the Version `version` is of, or
    /// else their own.
    async fn kept(&self, user: i64, version: i64, kind: &str) -> Result<Option<Row>, sqlx::Error> {
        // ADR 0035: the User's own applies wherever no Show's or Movie's does, an Unmatched
        // Item's included.
        let kept_for = match self.kept_for(version).await {
            Ok(kept_for) => kept_for.unwrap_or(KeptFor::USERS_OWN),
            Err(Error::NoSuchVersion) => KeptFor::USERS_OWN,
            Err(Error::Database(err)) => return Err(err),
        };
        self.row(user, kept_for, kind).await
    }

    /// The User `user`'s `kind` Preference for `kept_for`, or else their own.
    async fn row(
        &self,
        user: i64,
        KeptFor { scope, title_id }: KeptFor,
        kind: &str,
    ) -> Result<Option<Row>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT scope = ? AS "users_own!: bool", off AS "off: bool", language,
                      forced AS "forced: bool", sdh AS "sdh: bool", title,
                      foreign_audio_only AS "foreign_audio_only: bool"
               FROM track_preferences
               WHERE user_id = ? AND kind = ? AND ((scope = ? AND title_id = ?) OR scope = ?)
               ORDER BY scope = ?
               LIMIT 1"#,
            USER,
            user,
            kind,
            scope,
            title_id,
            USER,
            USER,
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(row.map(|row| Row {
            preference: (!row.off).then_some(Description {
                language: row.language,
                forced: row.forced,
                sdh: row.sdh,
                title: row.title,
            }),
            users_own: row.users_own,
            foreign_audio_only: row.foreign_audio_only,
        }))
    }

    /// The Show or Movie the Version `version` is of, which a Preference picked in it is kept
    /// for. `None` for an Unmatched Item, which keeps none.
    pub(crate) async fn kept_for(&self, version: i64) -> Result<Option<KeptFor>, Error> {
        Ok(match self.contents.matched_to(version).await? {
            MatchedTo::NoSuchVersion => return Err(Error::NoSuchVersion),
            MatchedTo::Nothing => None,
            MatchedTo::Movie(movie) => Some(KeptFor {
                scope: "movie",
                title_id: movie.to_string(),
            }),
            MatchedTo::Episodes { show, .. } => show.map(|show| KeptFor {
                scope: "show",
                title_id: show.to_string(),
            }),
        })
    }
}

/// The Subtitle Preference `row` keeps, and whose it is.
fn preferred_subtitle(row: Row) -> Preferred<SubtitlePreference> {
    Preferred {
        preference: row
            .preference
            .map_or(SubtitlePreference::Off, SubtitlePreference::Like),
        users_own: row.users_own,
        foreign_audio_only: row.foreign_audio_only,
    }
}

/// What describes the Track `preference` wants, `None` for off.
fn like(preference: SubtitlePreference) -> Option<Description> {
    match preference {
        SubtitlePreference::Off => None,
        SubtitlePreference::Like(description) => Some(description),
    }
}

/// Whether `code` is an ISO 639-1, 639-2 or 639-3 code of a language, which a User's own
/// Preference names its language by. ISO 639's codes for no particular language, such as `und`
/// and `mul`, aren't.
pub(crate) fn is_language(code: &str) -> bool {
    const NO_PARTICULAR_LANGUAGE: [&str; 4] = ["mis", "mul", "und", "zxx"];
    let none = NO_PARTICULAR_LANGUAGE
        .iter()
        .any(|no| no.eq_ignore_ascii_case(code));
    !none && language(code).is_some()
}

/// Whether the language tags `a` and `b` name the same language, whichever ISO 639 code each
/// uses, so `eng` is `en`. A Track with no language matches only another with none.
pub(crate) fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
    match (tagged(a), tagged(b)) {
        (None, None) => true,
        (Some(a), Some(b)) => match (language(a), language(b)) {
            (Some(a), Some(b)) => a == b,
            // A tag that isn't ISO 639, such as `pt-BR`, matches only itself.
            _ => a.eq_ignore_ascii_case(b),
        },
        _ => false,
    }
}

/// `tag`, or `None` for ISO 639-2's "undetermined", which many files tag a Track with in place of
/// none.
fn tagged(tag: Option<&str>) -> Option<&str> {
    tag.filter(|tag| !tag.eq_ignore_ascii_case("und"))
}

/// The language the ISO 639-1, 639-2 or 639-3 code `code` names.
fn language(code: &str) -> Option<Language> {
    let code = code.to_ascii_lowercase();
    let code = (BIBLIOGRAPHIC.iter())
        .find(|(bibliographic, _)| *bibliographic == code)
        .map_or(code.as_str(), |(_, terminology)| terminology);
    Language::from_639_1(code).or_else(|| Language::from_639_3(code))
}

/// The ISO 639-2/B codes, which files often carry, that differ from the 639-2/T code of the same
/// language, each with that code. isolang knows only the /T ones, which 639-3 shares. ISO froze
/// the /B list, so it never grows.
const BIBLIOGRAPHIC: [(&str, &str); 20] = [
    ("alb", "sqi"),
    ("arm", "hye"),
    ("baq", "eus"),
    ("bur", "mya"),
    ("chi", "zho"),
    ("cze", "ces"),
    ("dut", "nld"),
    ("fre", "fra"),
    ("geo", "kat"),
    ("ger", "deu"),
    ("gre", "ell"),
    ("ice", "isl"),
    ("mac", "mkd"),
    ("mao", "mri"),
    ("may", "msa"),
    ("per", "fas"),
    ("rum", "ron"),
    ("slo", "slk"),
    ("tib", "bod"),
    ("wel", "cym"),
];

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        Description, Preferred, SubtitlePreference, TrackPreferences, is_language, same_language,
    };
    use crate::{
        contents::{
            Contents, Decided,
            fixture::{MatchedMovie, tmdb_movie},
        },
        db::Database,
        plugins::Registry,
        users::{Permissions, User, Users},
    };

    /// Track Preferences on a database in a temp folder, and a User to keep them for.
    async fn track_preferences(data: &tempfile::TempDir) -> (TrackPreferences, i64) {
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let root = User {
            id: 0,
            name: "Root".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        };
        let plugins = Registry::new();
        plugins
            .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
            .unwrap();
        let user = Users::new(Arc::clone(&db), plugins.extension_points())
            .create(&root, "Ada", "correct horse battery staple")
            .await
            .unwrap();
        (TrackPreferences::new(db), user)
    }

    /// Each `track_preferences` row, as `(scope, title_id, kind, off, language, forced, sdh,
    /// title)`.
    async fn rows(
        preferences: &TrackPreferences,
    ) -> Vec<(
        String,
        String,
        String,
        i64,
        Option<String>,
        i64,
        i64,
        Option<String>,
    )> {
        sqlx::query_as(
            "SELECT scope, title_id, kind, off, language, forced, sdh, title
             FROM track_preferences",
        )
        .fetch_all(preferences.db.reader())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_users_own_audio_language_is_kept_as_their_untitled_audio_row_until_the_files_default()
     {
        let data = tempfile::tempdir().unwrap();
        let (preferences, user) = track_preferences(&data).await;

        preferences
            .keep_users_audio(user, Some("de".to_owned()))
            .await
            .unwrap();
        let kept = rows(&preferences).await;
        let read = preferences.users_audio(user).await.unwrap();
        preferences.keep_users_audio(user, None).await.unwrap();

        let german = (
            "user".to_owned(),
            String::new(),
            "audio".to_owned(),
            0,
            Some("de".to_owned()),
            0,
            0,
            None,
        );
        assert_eq!(kept, [german]);
        assert_eq!(read.as_deref(), Some("de"));
        assert_eq!(rows(&preferences).await, []);
        assert_eq!(preferences.users_audio(user).await.unwrap(), None);
    }

    // An Unmatched Item's Version can't be planned yet (`Contents::version_file`), so this is
    // where the User's own is seen to reach one.
    #[tokio::test]
    async fn the_users_own_audio_preference_reaches_a_version_of_an_unmatched_item() {
        let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (preferences, user) = track_preferences(&data).await;
        let alien =
            MatchedMovie::new(&preferences.db, media.path(), tmdb_movie(348, "Alien")).await;
        Contents::new(Arc::clone(&preferences.db))
            .record(vec![(alien.item, Decided::Unmatched(None))])
            .await
            .unwrap();

        preferences
            .keep_users_audio(user, Some("de".to_owned()))
            .await
            .unwrap();

        assert_eq!(
            preferences.audio(user, alien.version).await.unwrap(),
            Some(Preferred {
                preference: Description {
                    language: Some("de".to_owned()),
                    ..Description::default()
                },
                users_own: true,
                foreign_audio_only: false,
            })
        );
    }

    /// Keeps English subtitles as the User `user`'s own, as the Server did before the switch
    /// "Only when the audio is in another language" existed.
    async fn keep_english_subtitles_as_before_the_switch(
        preferences: &TrackPreferences,
        user: i64,
    ) {
        (preferences.db)
            .write(move |tx| {
                Box::pin(async move {
                    let insert = sqlx::query(
                        "INSERT INTO track_preferences
                             (user_id, scope, title_id, kind, off, language, forced, sdh)
                         VALUES (?, 'user', '', 'subtitle', 0, 'en', 0, 0)",
                    );
                    insert.bind(user).execute(&mut **tx).await
                })
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_users_own_subtitles_kept_before_the_condition_start_over_any_audio_until_it_is_set()
    {
        let data = tempfile::tempdir().unwrap();
        let (preferences, user) = track_preferences(&data).await;
        let english = SubtitlePreference::Like(Description {
            language: Some("en".to_owned()),
            ..Description::default()
        });
        keep_english_subtitles_as_before_the_switch(&preferences, user).await;
        let foreign_audio_only = async || {
            let preferred = preferences.users_subtitle(user).await.unwrap().unwrap();
            preferred.foreign_audio_only
        };

        let before = foreign_audio_only().await;
        preferences
            .keep_users_subtitle(user, english, true)
            .await
            .unwrap();

        assert!(!before);
        assert!(foreign_audio_only().await);
    }

    #[test]
    fn a_language_is_named_by_any_iso_639_code_of_one_language() {
        for code in ["en", "eng", "ger", "deu", "DE", "tlh"] {
            assert!(is_language(code), "{code}");
        }
        for code in ["klingon", "und", "mul", "zxx", "mis", "pt-BR", ""] {
            assert!(!is_language(code), "{code}");
        }
    }

    #[test]
    fn languages_match_whichever_iso_639_code_names_them() {
        for (a, b) in [
            ("eng", "en"),
            ("ger", "deu"),
            ("ger", "de"),
            ("fre", "fra"),
            ("chi", "zh"),
            ("per", "fa"),
            ("ENG", "en"),
            ("pt-BR", "pt-br"),
        ] {
            assert!(same_language(Some(a), Some(b)), "{a} is {b}");
        }
    }

    #[test]
    fn different_languages_and_a_missing_one_dont_match() {
        assert!(!same_language(Some("eng"), Some("ger")));
        assert!(!same_language(Some("pt-BR"), Some("pt")));
        assert!(!same_language(Some("eng"), None));
        assert!(!same_language(None, Some("eng")));
        assert!(same_language(None, None));
        assert!(same_language(Some("und"), None));
        assert!(!same_language(Some("und"), Some("eng")));
    }
}
