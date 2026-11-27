//! Where a subscriber reads the Content Rating of a Request's or Followed Show's title from.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use futures_util::future::BoxFuture;
use nama_proto::nama::v1::{Event, event};
use tracing::{error, warn};

use super::{About, Viewer, classify, lock};
use crate::{db::Database, users};

/// Reads the Content Rating of the title the Item it's given is Matched to, `None` for one
/// without, failing if the Item is no longer Matched to a title of the name it's given, so a
/// re-Match before the event is delivered can't judge it by another title's. The Library keeps
/// Matches, above this module, so a subscriber hands this in.
// ponytail: a re-Match to a title of the same name is judged by that title's rating; carry the
// rating beside each Event on the bus if that ever matters.
pub(crate) type MatchRatings = Arc<
    dyn Fn(i64, String) -> BoxFuture<'static, Result<Option<String>, sqlx::Error>> + Send + Sync,
>;

/// Reads no Match's Content Rating, for a subscriber whose Maximum Age lets every title through.
#[cfg(test)]
pub(super) fn unread_matches() -> MatchRatings {
    Arc::new(|_, _| Box::pin(async { Ok(None) }))
}

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
/// or once that is gone, what [`super::EventBus::removing`] kept of it; and of a Match's title,
/// from `matches`.
pub(super) struct Ratings {
    pub(super) db: Arc<Database>,
    pub(super) removed: Arc<Mutex<VecDeque<Removed>>>,
    pub(super) matches: MatchRatings,
}

impl Ratings {
    /// The Content Rating of the title named `title` that the Item `item` is Matched to, as
    /// [`MatchRatings`] reads it.
    pub(super) async fn of_match(
        &self,
        item: i64,
        title: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        (self.matches)(item, title.to_owned()).await
    }

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

impl Viewer {
    /// Whether this viewer's Reach lets the title of `event` reach them: one about another User's Request
    /// or Followed Show, or any `ItemMatched`, of a title over it doesn't, as their lists leave
    /// those out. One whose rating can't be read, logged, doesn't either.
    pub(super) async fn allows_title_of(&self, ratings: &Ratings, event: &Event) -> bool {
        if self.reach.lets_every_rating() {
            return true;
        }
        if let Some(event::Event::ItemMatched(matched)) = &event.event {
            return match ratings.of_match(matched.item_id, &matched.title).await {
                Ok(rating) => self.reach.allows_rating(rating.as_deref()),
                Err(err) => {
                    warn!(
                        item = matched.item_id,
                        "a subscriber with a Maximum Age wasn't told of a Match whose Content \
                         Rating couldn't be read: {err}"
                    );
                    false
                }
            };
        }
        let row = match classify(event) {
            Some((_, About::Request(request)))
                if !users::is_requester(self.user, &request.requesters) =>
            {
                Rated::Request(request.id)
            }
            Some((_, About::FollowedShow(followed)))
                if !users::is_requester(self.user, &followed.requesters) =>
            {
                Rated::FollowedShow(followed.id)
            }
            _ => return true,
        };
        match ratings.of(row).await {
            Ok(Some(rating)) => self.reach.allows_rating(rating.as_deref()),
            Ok(None) => false,
            Err(err) => {
                error!("could not read the Content Rating of {row:?} for a subscriber: {err}");
                false
            }
        }
    }
}
