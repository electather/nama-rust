//! Where a subscriber reads the Content Rating of a Request's or Followed Show's title from.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use super::lock;
use crate::db::Database;

/// A Request or Followed Show, by id, whose title's Content Rating a Maximum Age judges events
/// about it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rated {
    Request(i64),
    FollowedShow(i64),
}

/// A Request or Followed Show a write removes, with the Content Rating its row stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Removed {
    pub(crate) row: Rated,
    pub(crate) content_rating: Option<String>,
}

/// Where a subscriber reads the Content Rating of a Request's or Followed Show's title: its row,
/// or once that is gone, what [`super::EventBus::removing`] kept of it.
#[derive(Debug)]
pub(super) struct Ratings {
    pub(super) db: Arc<Database>,
    pub(super) removed: Arc<Mutex<VecDeque<Removed>>>,
}

impl Ratings {
    /// The Content Rating stored for `row`, `None` if neither its row nor
    /// [`super::EventBus::removing`] has it any more.
    pub(super) async fn of(&self, row: Rated) -> Result<Option<Option<String>>, sqlx::Error> {
        // The Request and Followed Show protos carry no Content Rating, so it's read from the row.
        let stored = match row {
            Rated::Request(id) => {
                sqlx::query_scalar!("SELECT content_rating FROM requests WHERE id = ?", id)
                    .fetch_optional(self.db.reader())
                    .await?
            }
            Rated::FollowedShow(id) => {
                sqlx::query_scalar!("SELECT content_rating FROM followed_shows WHERE id = ?", id)
                    .fetch_optional(self.db.reader())
                    .await?
            }
        };
        Ok(stored.or_else(|| {
            let kept = lock(&self.removed);
            let found = kept.iter().rev().find(|removed| removed.row == row);
            found.map(|removed| removed.content_rating.clone())
        }))
    }
}
