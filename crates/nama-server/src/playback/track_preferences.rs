//! Track Preferences: what a User wants a kind of Track to start as when they play something (ADR
//! 0035): the Subtitle and Audio Preferences a User's pick keeps for a Show or Movie, and the
//! User's own, set in Settings.
//!
//! [`TrackPreferences`] alone reads and writes `track_preferences` (`AGENTS.md`). It keeps
//! a pick's under each of a Show's or Movie's external ids (ADR 0013), so an Unmatched Item keeps
//! none and one pulled under any of them is found, and the User's own at the `user` scope. How
//! they sync with the User's other Servers: `AGENTS.md` (ADR 0092).

use std::sync::Arc;

use isolang::Language;
use nama_plugin_api::{
    metadata::ExternalIds,
    points::{
        TrackDescription, TrackPreference, TrackPreferenceChange, TrackPreferenceChanged,
        TrackPreferenceFor,
    },
};
use sqlx::SqliteConnection;

use super::tracks::Track;
use crate::{
    db::{Database, detached, now_ms},
    library::{Contents, MatchedTo},
    plugins::ExtensionPoints,
};

// The `kind` of a Preference, part of its key.
const SUBTITLE: &str = "subtitle";
const AUDIO: &str = "audio";

// The `scope` of a Show's or Movie's Preference.
const SHOW: &str = "show";
const MOVIE: &str = "movie";

/// Reads and keeps each User's Track Preferences.
#[derive(Debug, Clone)]
pub(crate) struct TrackPreferences {
    db: Arc<Database>,
    contents: Contents,
    /// Told of each Preference kept, such as the Hub Plugin's, which syncs them (ADR 0092).
    points: ExtensionPoints,
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

/// What a Preference is kept for: a Show or Movie, by its scope and external ids, or everything
/// the User plays. The key after the User.
#[derive(Debug)]
pub(crate) struct KeptFor {
    scope: &'static str,
    /// Each a row's `title_id`; none for the User's own, kept under ''.
    title_ids: Vec<String>,
}

impl KeptFor {
    /// Everything the User plays: their own Preference, set in Settings.
    pub(super) const USERS_OWN: Self = Self {
        scope: USER,
        title_ids: Vec::new(),
    };

    /// The Show or Movie `scope` known by every id of `ids`; `None` for one with none.
    fn of_title(scope: &'static str, ids: &ExternalIds) -> Option<Self> {
        let title_ids: Vec<_> = ids.iter().map(|id| id.to_string()).collect();
        (!title_ids.is_empty()).then_some(Self { scope, title_ids })
    }

    /// What `kept_for`, as another Server named it, is here. `None` for a Show or Movie with no id.
    fn of(kept_for: &TrackPreferenceFor) -> Option<Self> {
        let (scope, title_ids) = match kept_for {
            TrackPreferenceFor::User => return Some(Self::USERS_OWN),
            TrackPreferenceFor::Show(ids) => (SHOW, ids),
            TrackPreferenceFor::Movie(ids) => (MOVIE, ids),
        };
        let title_ids: Vec<_> = (title_ids.iter())
            .filter(|id| !id.is_empty())
            .cloned()
            .collect();
        (!title_ids.is_empty()).then_some(Self { scope, title_ids })
    }

    /// It, as the Plugins that sync Preferences name it.
    fn told(&self) -> TrackPreferenceFor {
        match self.scope {
            SHOW => TrackPreferenceFor::Show(self.title_ids.clone()),
            MOVIE => TrackPreferenceFor::Movie(self.title_ids.clone()),
            _ => TrackPreferenceFor::User,
        }
    }

    /// Its rows' `title_id`s, as a JSON array for `json_each`.
    fn title_ids_json(&self) -> String {
        let title_ids: Vec<&str> = match self.title_ids.as_slice() {
            [] => vec![""],
            title_ids => title_ids.iter().map(String::as_str).collect(),
        };
        serde_json::Value::from(title_ids).to_string()
    }
}

// The scope of a User's own Preference, kept with an empty `title_id`.
const USER: &str = "user";

/// A `track_preferences` row: what describes the Track it wants, `None` for off, and whose it is.
type Row = Preferred<Option<Description>>;

/// One change to a User's `track_preferences`: the rows it keeps, after the User.
#[derive(Debug)]
struct Change {
    kept_for: KeptFor,
    kind: &'static str,
    /// What describes the Track wanted, `None` for off, or, for the User's own audio, for none
    /// kept, as audio is never off.
    like: Option<Description>,
    foreign_audio_only: bool,
    /// In milliseconds since the Unix epoch.
    changed_at: i64,
}

impl Change {
    /// `pulled`, as this Server keeps it, `None` for one its rows can't hold, which no Server
    /// makes.
    fn pulled(pulled: &TrackPreference) -> Option<Self> {
        let kept_for = KeptFor::of(&pulled.kept_for)?;
        let users_own = kept_for.scope == USER;
        // Audio is never off, and a Show's or Movie's is always a Track's.
        if pulled.audio && pulled.like.is_none() && !users_own {
            return None;
        }
        let like = pulled.like.clone().map(|like| Description {
            language: like.language,
            // Audio is described by its language and title alone.
            forced: like.forced && !pulled.audio,
            sdh: like.sdh && !pulled.audio,
            title: like.title,
        });
        Some(Self {
            kept_for,
            kind: if pulled.audio { AUDIO } else { SUBTITLE },
            like,
            foreign_audio_only: pulled.foreign_audio_only && users_own && !pulled.audio,
            changed_at: pulled.changed_at_ms,
        })
    }

    /// It, as the Plugins that sync Preferences are told.
    fn told(self) -> TrackPreference {
        TrackPreference {
            audio: self.kind == AUDIO,
            kept_for: self.kept_for.told(),
            like: self.like.map(|like| TrackDescription {
                language: like.language,
                forced: like.forced,
                sdh: like.sdh,
                title: like.title,
            }),
            foreign_audio_only: self.foreign_audio_only,
            changed_at_ms: self.changed_at,
        }
    }
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
    pub(crate) fn new(db: Arc<Database>, points: ExtensionPoints) -> Self {
        Self {
            contents: Contents::new(Arc::clone(&db)),
            db,
            points,
        }
    }

    /// Keeps `preference` as the User `user`'s Subtitle Preference for `kept_for`, in place of any
    /// they had.
    pub(crate) async fn keep_subtitle(
        &self,
        user: i64,
        kept_for: KeptFor,
        preference: SubtitlePreference,
    ) -> Result<TrackPreference, sqlx::Error> {
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
    ) -> Result<TrackPreference, sqlx::Error> {
        let like = like(preference);
        self.keep(user, KeptFor::USERS_OWN, SUBTITLE, like, foreign_audio_only)
            .await
    }

    /// Keeps a Track like `picked` as the User `user`'s Audio Preference for `kept_for`, in place
    /// of any they had. Only its language and title are kept: forced and SDH describe subtitles.
    pub(crate) async fn keep_audio(
        &self,
        user: i64,
        kept_for: KeptFor,
        picked: Description,
    ) -> Result<TrackPreference, sqlx::Error> {
        let description = Description {
            language: picked.language,
            title: picked.title,
            ..Description::default()
        };
        self.keep(user, kept_for, AUDIO, Some(description), false)
            .await
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
    ) -> Result<TrackPreference, sqlx::Error> {
        let Some(language) = language else {
            return self
                .keep(user, KeptFor::USERS_OWN, AUDIO, None, false)
                .await;
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

    /// Keeps a Track like `like`, or `None` for off, as the User `user`'s `kind` Preference for
    /// `kept_for`, wanting it only over audio in another language if `foreign_audio_only`, tells
    /// `TrackPreferenceChanged` Observers, and returns the change. `None` for the User's own audio
    /// forgets it, as audio is never off. They're told in a task of its own, so they're told even
    /// if the caller stops waiting once the write is sent (ADR 0029).
    #[expect(
        clippy::too_many_arguments,
        reason = "the row's key and columns, each set by a different public `keep_*`"
    )]
    async fn keep(
        &self,
        user: i64,
        kept_for: KeptFor,
        kind: &'static str,
        like: Option<Description>,
        foreign_audio_only: bool,
    ) -> Result<TrackPreference, sqlx::Error> {
        let change = Change {
            kept_for,
            kind,
            like,
            foreign_audio_only,
            changed_at: now_ms(),
        };
        let (db, points) = (Arc::clone(&self.db), self.points.clone());
        detached(async move {
            let preference = (db.write(move |tx| Box::pin(keep_change(tx, user, change)))).await?;
            points.notify::<TrackPreferenceChanged>(TrackPreferenceChange {
                user_id: user,
                preference: preference.clone(),
            });
            Ok(preference)
        })
        .await
    }

    /// Takes each of `pulled`, the User `user`'s Preferences as their other Servers left them,
    /// where it is newer than this Server's own. It tells no Plugin: the one that pulled has them.
    pub(crate) async fn take_pulled(
        &self,
        user: i64,
        pulled: Vec<TrackPreference>,
    ) -> Result<(), sqlx::Error> {
        let changes: Vec<_> = pulled.iter().filter_map(Change::pulled).collect();
        self.db
            .write(move |tx| Box::pin(take_each(tx, user, changes)))
            .await
    }

    /// Every Track Preference of the User `user`, each as the change that kept it, as Plugins are
    /// told of it: a Show's or Movie's once per id it is kept under, as each row holds one. Each is
    /// dated as stored, never now, so a later change elsewhere wins.
    pub(crate) async fn changes_of_user(
        &self,
        user: i64,
    ) -> Result<Vec<TrackPreference>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT scope, title_id, kind, off AS "off: bool", language,
                      forced AS "forced: bool", sdh AS "sdh: bool", title,
                      foreign_audio_only AS "foreign_audio_only: bool", changed_at
               FROM track_preferences
               WHERE user_id = ?"#,
            user,
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok((rows.into_iter())
            .map(|row| TrackPreference {
                audio: row.kind == AUDIO,
                kept_for: match row.scope.as_str() {
                    SHOW => TrackPreferenceFor::Show(vec![row.title_id]),
                    MOVIE => TrackPreferenceFor::Movie(vec![row.title_id]),
                    _ => TrackPreferenceFor::User,
                },
                like: (!row.off).then_some(TrackDescription {
                    language: row.language,
                    forced: row.forced,
                    sdh: row.sdh,
                    title: row.title,
                }),
                foreign_audio_only: row.foreign_audio_only,
                changed_at_ms: row.changed_at,
            })
            .collect())
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

    /// The User `user`'s `kind` Preference for `kept_for`, under whichever of its ids changed
    /// last, or else their own.
    async fn row(
        &self,
        user: i64,
        kept_for: KeptFor,
        kind: &str,
    ) -> Result<Option<Row>, sqlx::Error> {
        let title_ids = kept_for.title_ids_json();
        let row = sqlx::query!(
            r#"SELECT scope = ? AS "users_own!: bool", off AS "off: bool", language,
                      forced AS "forced: bool", sdh AS "sdh: bool", title,
                      foreign_audio_only AS "foreign_audio_only: bool"
               FROM track_preferences
               WHERE user_id = ? AND kind = ?
                 AND ((scope = ? AND title_id IN (SELECT value FROM json_each(?))) OR scope = ?)
               ORDER BY scope = ?, changed_at DESC
               LIMIT 1"#,
            USER,
            user,
            kind,
            kept_for.scope,
            title_ids,
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
            MatchedTo::Movie(ids) => KeptFor::of_title(MOVIE, &ids),
            MatchedTo::Episodes { show } => show.and_then(|ids| KeptFor::of_title(SHOW, &ids)),
        })
    }
}

/// When a write replaces the row kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overwrite {
    /// For the User's own change here, which wins even over a pulled one dated later by another
    /// Server's clock.
    Always,
    /// For a pulled change, taken only where it is newer.
    IfNewer,
}

/// Writes `change` of the User `user`'s Preferences over whatever they had, and returns it as
/// Observers are told of it.
async fn keep_change(
    conn: &mut SqliteConnection,
    user: i64,
    change: Change,
) -> Result<TrackPreference, sqlx::Error> {
    write(conn, user, &change, Overwrite::Always).await?;
    Ok(change.told())
}

/// Writes each of `changes`, pulled, to the User `user`'s `track_preferences` through `conn`.
async fn take_each(
    conn: &mut SqliteConnection,
    user: i64,
    changes: Vec<Change>,
) -> Result<(), sqlx::Error> {
    for change in &changes {
        write(conn, user, change, Overwrite::IfNewer).await?;
    }
    Ok(())
}

/// Writes `change` to the User `user`'s `track_preferences` through `conn`, as `overwrite` says.
async fn write(
    conn: &mut SqliteConnection,
    user: i64,
    change: &Change,
    overwrite: Overwrite,
) -> Result<(), sqlx::Error> {
    let scope = change.kept_for.scope;
    let title_ids = change.kept_for.title_ids_json();
    let always = overwrite == Overwrite::Always;
    if change.kind == AUDIO && change.like.is_none() {
        sqlx::query!(
            "DELETE FROM track_preferences
             WHERE user_id = ? AND scope = ? AND title_id IN (SELECT value FROM json_each(?))
                 AND kind = ? AND (? OR changed_at < ?)",
            user,
            scope,
            title_ids,
            change.kind,
            always,
            change.changed_at,
        )
        .execute(conn)
        .await?;
        return Ok(());
    }
    let off = change.like.is_none();
    let description = change.like.clone().unwrap_or_default();
    // `WHERE true` tells SQLite the `ON CONFLICT` is the upsert's, not the join's.
    sqlx::query!(
        "INSERT INTO track_preferences
             (user_id, scope, title_id, kind, off, language, forced, sdh, title,
              foreign_audio_only, changed_at)
         SELECT ?, ?, value, ?, ?, ?, ?, ?, ?, ?, ? FROM json_each(?) WHERE true
         ON CONFLICT (user_id, scope, title_id, kind) DO UPDATE
         SET off = excluded.off, language = excluded.language, forced = excluded.forced,
             sdh = excluded.sdh, title = excluded.title,
             foreign_audio_only = excluded.foreign_audio_only, changed_at = excluded.changed_at
         WHERE ? OR excluded.changed_at > track_preferences.changed_at",
        user,
        scope,
        change.kind,
        off,
        description.language,
        description.forced,
        description.sdh,
        description.title,
        change.foreign_audio_only,
        change.changed_at,
        title_ids,
        always,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// The Subtitle Preference `row` keeps, and whose it is.
fn preferred_subtitle(row: Row) -> Preferred<SubtitlePreference> {
    Preferred {
        preference: row
            .preference
            .map_or(SubtitlePreference::Off, SubtitlePreference::Like),
        users_own: row.users_own,
        // ADR 0035: a Show's or Movie's has no condition, whatever its row holds.
        foreign_audio_only: row.users_own && row.foreign_audio_only,
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
pub(super) fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
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
pub(super) fn language(code: &str) -> Option<Language> {
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
#[path = "track_preferences_tests.rs"]
mod tests;
