//! What the Hub recommends to a Linked User, for their Featured and Home rows (ADR 0072): the titles
//! nearest the taste vector of the Account they are Linked to, which the titles its Viewing Events
//! finished make (ADR 0093, 0095).

use sqlx::PgPool;

use crate::{
    accounts::AccountId,
    links::{LinkedServer, Links},
    titles::{self, ExternalId, RecommendedTitle, TitleKind},
    viewing::Viewing,
};

/// Every Linked User's recommendations.
#[derive(Clone)]
pub(crate) struct Recommendations {
    db: PgPool,
    viewing: Viewing,
}

impl Recommendations {
    pub(crate) const fn new(db: PgPool, viewing: Viewing) -> Self {
        Self { db, viewing }
    }

    /// What [`Self::for_account`] recommends the Account `links` finds `server`'s User `user_id`
    /// Linked to. Empty while they aren't Linked.
    pub(crate) async fn for_user(
        &self,
        links: &Links,
        server: &LinkedServer,
        user_id: i64,
        namespaces: &[String],
    ) -> Result<Vec<RecommendedTitle>, sqlx::Error> {
        match links.linked_account(server, user_id).await? {
            Some(account) => self.for_account(account, namespaces).await,
            None => Ok(Vec::new()),
        }
    }

    /// The titles to recommend to `account`, best first, by their ids in the first of
    /// `namespaces` each has, leaving out what it finished.
    pub(crate) async fn for_account(
        &self,
        account: AccountId,
        namespaces: &[String],
    ) -> Result<Vec<RecommendedTitle>, sqlx::Error> {
        // ponytail: an Episode's Viewing Event names the Episode, not its Show, and the Hub's titles
        // hold no Episodes, so only Movies count; count Shows once an event names its Show (#1950).
        let finished: Vec<(TitleKind, ExternalId)> =
            (self.viewing.finished_movies(account).await?)
                .into_iter()
                .map(|movie| (TitleKind::Movie, movie))
                .collect();
        titles::recommended(&self.db, &finished, namespaces).await
    }
}
