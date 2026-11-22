//! Why an Item is Unmatched, as the columns of `item_matches` store it.

use super::{TiedTitle, UnmatchedReason, parse_hint};

/// An [`UnmatchedReason`] as `item_matches.unmatched_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub(super) enum ReasonColumn {
    Ambiguous,
    NoTitleFits,
    UnknownIdHint,
    NotInShow,
}

impl ReasonColumn {
    pub(super) const fn of(reason: &UnmatchedReason) -> Self {
        match reason {
            UnmatchedReason::Ambiguous(_) => Self::Ambiguous,
            UnmatchedReason::NoTitleFits => Self::NoTitleFits,
            UnmatchedReason::UnknownIdHint => Self::UnknownIdHint,
            UnmatchedReason::NotInShow => Self::NotInShow,
        }
    }

    /// The reason this column and `tied_titles`, the JSON the migration describes, store.
    pub(super) fn reason(self, tied_titles: Option<&str>) -> Result<UnmatchedReason, sqlx::Error> {
        Ok(match self {
            Self::Ambiguous => UnmatchedReason::Ambiguous(parse_tied(tied_titles.unwrap_or("[]"))?),
            Self::NoTitleFits => UnmatchedReason::NoTitleFits,
            Self::UnknownIdHint => UnmatchedReason::UnknownIdHint,
            Self::NotInShow => UnmatchedReason::NotInShow,
        })
    }
}

fn parse_tied(json: &str) -> Result<Vec<TiedTitle>, sqlx::Error> {
    let rows: Vec<(String, String, Option<String>, Option<i32>)> =
        serde_json::from_str(json).map_err(|err| sqlx::Error::Decode(err.into()))?;
    rows.into_iter()
        .map(|(id, title, original_title, year)| {
            let bad = || sqlx::Error::Decode(format!("bad tied title id {id}").into());
            let id = parse_hint(&id).ok_or_else(bad)?;
            Ok(TiedTitle {
                id,
                title,
                original_title,
                year,
            })
        })
        .collect()
}

pub(super) fn tied_json(tied: &[TiedTitle]) -> String {
    let rows: Vec<serde_json::Value> = tied
        .iter()
        .map(|t| serde_json::json!([t.id.to_string(), t.title, t.original_title, t.year]))
        .collect();
    serde_json::Value::from(rows).to_string()
}
