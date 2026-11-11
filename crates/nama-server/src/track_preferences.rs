//! Track Preferences: what a User wants a kind of Track to start as when they play something (ADR
//! 0035). So far only the Subtitle Preference a User's pick keeps for a Show or Movie.
//!
//! [`TrackPreferences`] is the only code that reads or writes `track_preferences`. It keeps them
//! against a Show's or Movie's external id, as Watch State is kept (ADR 0013), so an Unmatched
//! Item keeps none.

use std::sync::Arc;

use isolang::Language;

use crate::{
    contents::{Contents, MatchedTo},
    db::Database,
    tracks::Track,
};

// The `kind` of a Preference, part of its key; audio is to come (#691).
const SUBTITLE: &str = "subtitle";

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

/// Why a Preference couldn't be kept.
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

    /// Keeps `preference` as the User `user`'s Subtitle Preference for the Show or Movie the
    /// Version `version` is of, in place of any they had. Keeps nothing for an Unmatched Item.
    pub(crate) async fn keep_subtitle(
        &self,
        user: i64,
        version: i64,
        preference: SubtitlePreference,
    ) -> Result<(), Error> {
        let Some((scope, title_id)) = self.kept_for(version).await? else {
            return Ok(());
        };
        let (off, description) = match preference {
            SubtitlePreference::Off => (true, Description::default()),
            SubtitlePreference::Like(description) => (false, description),
        };
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
                        SUBTITLE,
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
            .await?;
        Ok(())
    }

    /// The User `user`'s Subtitle Preference for the Show or Movie the Version `version` is of,
    /// `None` if they have none, or it's of nothing that keeps one.
    pub(crate) async fn subtitle(
        &self,
        user: i64,
        version: i64,
    ) -> Result<Option<SubtitlePreference>, sqlx::Error> {
        let (scope, title_id) = match self.kept_for(version).await {
            Ok(Some(key)) => key,
            Ok(None) | Err(Error::NoSuchVersion) => return Ok(None),
            Err(Error::Database(err)) => return Err(err),
        };
        let row = sqlx::query!(
            r#"SELECT off AS "off: bool", language, forced AS "forced: bool", sdh AS "sdh: bool",
                      title
               FROM track_preferences
               WHERE user_id = ? AND scope = ? AND title_id = ? AND kind = ?"#,
            user,
            scope,
            title_id,
            SUBTITLE,
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(row.map(|row| {
            if row.off {
                SubtitlePreference::Off
            } else {
                SubtitlePreference::Like(Description {
                    language: row.language,
                    forced: row.forced,
                    sdh: row.sdh,
                    title: row.title,
                })
            }
        }))
    }

    /// The key, after the User, of a Preference for what the Version `version` is of: its scope
    /// and external id. `None` for an Unmatched Item.
    async fn kept_for(&self, version: i64) -> Result<Option<(&'static str, String)>, Error> {
        Ok(match self.contents.matched_to(version).await? {
            MatchedTo::NoSuchVersion => return Err(Error::NoSuchVersion),
            MatchedTo::Nothing => None,
            MatchedTo::Movie(movie) => Some(("movie", movie.to_string())),
            MatchedTo::Episodes { show, .. } => show.map(|show| ("show", show.to_string())),
        })
    }
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
    use super::same_language;

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
