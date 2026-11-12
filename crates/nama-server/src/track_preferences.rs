//! Track Preferences: what a User wants a kind of Track to start as when they play something (ADR
//! 0035). So far the Subtitle and Audio Preferences a User's pick keeps for a Show or Movie, and the
//! User's own Subtitle Preference, set in Settings.
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

/// The Subtitle Preference a plan starts with, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preferred {
    pub(crate) preference: SubtitlePreference,
    /// The User's own, set without a title, rather than the Show's or Movie's.
    pub(crate) users_own: bool,
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

/// A `track_preferences` row: whether it is off, what describes the Track it wants, and whether
/// it is the User's own.
struct Row {
    off: bool,
    description: Description,
    users_own: bool,
}

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
        let like = match preference {
            SubtitlePreference::Off => None,
            SubtitlePreference::Like(description) => Some(description),
        };
        self.keep(user, kept_for, SUBTITLE, like).await
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
        self.keep(user, kept_for, AUDIO, Some(description)).await
    }

    /// The Subtitle Preference a plan of the Version `version` starts with for the User `user`:
    /// the one kept for the Show or Movie it is of, which wins even when no Track matches it, or
    /// else their own. `None` if they have neither.
    pub(crate) async fn subtitle(
        &self,
        user: i64,
        version: i64,
    ) -> Result<Option<Preferred>, sqlx::Error> {
        let row = self.kept(user, version, SUBTITLE).await?;
        Ok(row.map(|row| Preferred {
            preference: subtitle_preference(row.off, row.description),
            users_own: row.users_own,
        }))
    }

    /// The User `user`'s own Subtitle Preference, set in Settings, `None` if they never set one.
    pub(crate) async fn users_subtitle(
        &self,
        user: i64,
    ) -> Result<Option<SubtitlePreference>, sqlx::Error> {
        let row = self.row(user, KeptFor::USERS_OWN, SUBTITLE).await?;
        Ok(row.map(|row| subtitle_preference(row.off, row.description)))
    }

    /// What describes the audio Track the User `user` wants for the Show or Movie the Version
    /// `version` is of: their Audio Preference, `None` if they have none, or it's of nothing that
    /// keeps one.
    pub(crate) async fn audio(
        &self,
        user: i64,
        version: i64,
    ) -> Result<Option<Description>, sqlx::Error> {
        let row = self.kept(user, version, AUDIO).await?;
        // Whether audio falls back to a User's own Audio Preference is #691's to decide.
        Ok(row.filter(|row| !row.users_own).map(|row| row.description))
    }

    /// Keeps a Track like `like`, or `None` for off, as the User `user`'s `kind` Preference for
    /// `kept_for`.
    async fn keep(
        &self,
        user: i64,
        KeptFor { scope, title_id }: KeptFor,
        kind: &'static str,
        like: Option<Description>,
    ) -> Result<(), sqlx::Error> {
        let off = like.is_none();
        let description = like.unwrap_or_default();
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "INSERT INTO track_preferences
                             (user_id, scope, title_id, kind, off, language, forced, sdh, title)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT (user_id, scope, title_id, kind) DO UPDATE
                         SET off = excluded.off, language = excluded.language,
                             forced = excluded.forced, sdh = excluded.sdh, title = excluded.title",
                        user,
                        scope,
                        title_id,
                        kind,
                        off,
                        description.language,
                        description.forced,
                        description.sdh,
                        description.title,
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
                      forced AS "forced: bool", sdh AS "sdh: bool", title
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
            off: row.off,
            description: Description {
                language: row.language,
                forced: row.forced,
                sdh: row.sdh,
                title: row.title,
            },
            users_own: row.users_own,
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

/// The Subtitle Preference a row that is `off`, or wants a Track like `description`, keeps.
fn subtitle_preference(off: bool, description: Description) -> SubtitlePreference {
    if off {
        SubtitlePreference::Off
    } else {
        SubtitlePreference::Like(description)
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
    use super::{is_language, same_language};

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
