//! Qualities (ADR 0025): the service quality profiles an admin offers on each Fulfiller's list,
//! under friendly names, with one the default a new Request takes.
//!
//! This module is the only code that reads or writes `qualities`.

use std::collections::{HashMap, HashSet};

use nama_plugin_api::PluginId;
use nama_proto::nama::v1::{Quality, RequestedQuality};
use sqlx::SqliteConnection;

use crate::db::Database;

/// The choice setting whose choices are the profiles a Fulfiller's Qualities may offer. A
/// Fulfiller takes a Request's profile in its place, so the setting stays the fallback.
pub(crate) const QUALITY_PROFILE: &str = "quality_profile";

/// The parts of a Quality a list can leave blank or repeat, by their proto field names.
pub(crate) const PROFILE: &str = "profile_id";
pub(crate) const NAME: &str = "name";

/// Why a list of Qualities can't be saved. The `usize` is the index of the Quality at fault.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Invalid {
    #[error("a Quality has no {1}")]
    Blank(usize, &'static str),
    #[error("two Qualities have the same {1}")]
    Repeated(usize, &'static str),
    #[error("exactly one Quality must be the default")]
    NotOneDefault,
}

/// Checks `qualities` against the list rules: each has a profile and a name, no profile or name
/// (ignoring case) is given twice, and a list with any has exactly one default.
pub(crate) fn check(qualities: &[Quality]) -> Result<(), Invalid> {
    let (mut profiles, mut names) = (HashSet::new(), HashSet::new());
    for (index, quality) in qualities.iter().enumerate() {
        let parts = [
            (PROFILE, quality.profile_id.clone(), &mut profiles),
            (NAME, quality.name.trim().to_lowercase(), &mut names),
        ];
        for (part, value, seen) in parts {
            if value.is_empty() {
                return Err(Invalid::Blank(index, part));
            }
            if !seen.insert(value) {
                return Err(Invalid::Repeated(index, part));
            }
        }
    }
    let defaults = qualities
        .iter()
        .filter(|quality| quality.is_default)
        .count();
    if qualities.is_empty() || defaults == 1 {
        Ok(())
    } else {
        Err(Invalid::NotOneDefault)
    }
}

/// The Plugin `plugin`'s Qualities, in the order the admin listed them.
pub(crate) async fn list(
    conn: &mut SqliteConnection,
    plugin: &PluginId,
) -> Result<Vec<Quality>, sqlx::Error> {
    let plugin = plugin.as_str();
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", profile_id, name, description, is_default AS "is_default: bool"
           FROM qualities WHERE plugin_id = ? ORDER BY position"#,
        plugin
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| Quality {
            id: row.id,
            profile_id: row.profile_id,
            name: row.name,
            description: row.description,
            is_default: row.is_default,
            ..Default::default()
        })
        .collect())
}

/// Replaces the Plugin `plugin`'s Qualities with `qualities`, which [`check`] passed, and returns
/// them as saved. A Quality keeps its id if `plugin`'s list has it; any other gets a new one.
pub(crate) async fn replace(
    db: &Database,
    plugin: PluginId,
    qualities: Vec<Quality>,
) -> Result<Vec<Quality>, sqlx::Error> {
    db.write(move |tx| {
        Box::pin(async move {
            let id = plugin.as_str();
            let listed: HashMap<i64, Quality> = list(tx, &plugin)
                .await?
                .into_iter()
                .map(|quality| (quality.id, quality))
                .collect();
            // Deleted first, so a name or profile moving between Qualities can't trip the UNIQUE
            // constraints halfway.
            sqlx::query!("DELETE FROM qualities WHERE plugin_id = ?", id)
                .execute(&mut **tx)
                .await?;
            for (position, quality) in (0_i64..).zip(&qualities) {
                let kept_id = listed.contains_key(&quality.id).then_some(quality.id);
                let name = quality.name.trim();
                let description = quality.description.trim();
                sqlx::query!(
                    "INSERT INTO qualities
                         (id, plugin_id, position, profile_id, name, description, is_default)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    kept_id,
                    id,
                    position,
                    quality.profile_id,
                    name,
                    description,
                    quality.is_default
                )
                .execute(&mut **tx)
                .await?;
            }
            list(tx, &plugin).await
        })
    })
    .await
}

/// A copy of the Plugin `plugin`'s default Quality, for a new Request, or `None` if its list is
/// empty.
pub(crate) async fn default_of(
    conn: &mut SqliteConnection,
    plugin: &PluginId,
) -> Result<Option<RequestedQuality>, sqlx::Error> {
    let plugin = plugin.as_str();
    let row = sqlx::query!(
        r#"SELECT id AS "id!", name, profile_id FROM qualities WHERE plugin_id = ? AND is_default"#,
        plugin
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|row| RequestedQuality {
        id: row.id,
        name: row.name,
        profile_id: row.profile_id,
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quality(profile_id: &str, name: &str, is_default: bool) -> Quality {
        Quality {
            profile_id: profile_id.to_owned(),
            name: name.to_owned(),
            is_default,
            ..Default::default()
        }
    }

    #[test]
    fn an_empty_list_or_one_with_one_default_and_nothing_repeated_passes() {
        let offered = [quality("4", "HD", true), quality("5", "4K", false)];

        assert_eq!(check(&[]), Ok(()));
        assert_eq!(check(&offered), Ok(()));
    }

    #[test]
    fn a_blank_name_or_profile_is_named_by_index() {
        let blank_name = [quality("4", "HD", true), quality("5", "  ", false)];
        let blank_profile = [quality("", "HD", true)];

        assert_eq!(check(&blank_name), Err(Invalid::Blank(1, NAME)));
        assert_eq!(check(&blank_profile), Err(Invalid::Blank(0, PROFILE)));
    }

    #[test]
    fn a_name_repeated_in_another_case_or_with_spaces_is_repeated() {
        let names = [quality("4", "HD", true), quality("5", " hd ", false)];

        assert_eq!(check(&names), Err(Invalid::Repeated(1, NAME)));
    }

    #[test]
    fn a_list_needs_exactly_one_default() {
        let none = [quality("4", "HD", false)];
        let two = [quality("4", "HD", true), quality("5", "4K", true)];

        assert_eq!(check(&none), Err(Invalid::NotOneDefault));
        assert_eq!(check(&two), Err(Invalid::NotOneDefault));
    }
}
