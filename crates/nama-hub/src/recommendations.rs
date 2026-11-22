//! What the Hub recommends to a Linked User, for their Featured and Home rows (ADR 0072): the titles
//! nearest the taste vector of the Account they are Linked to, which the titles its Viewing Events
//! finished make (ADR 0093, 0095).

use sqlx::PgPool;

use crate::{
    links::{LinkedServer, Links},
    titles::{self, ExternalId, RecommendedTitle, TitleKind},
    viewing::{Kind, Viewing},
};

/// Every Linked User's recommendations.
#[derive(Clone)]
pub(crate) struct Recommendations {
    db: PgPool,
    links: Links,
    viewing: Viewing,
}

impl Recommendations {
    pub(crate) const fn new(db: PgPool, links: Links, viewing: Viewing) -> Self {
        Self { db, links, viewing }
    }

    /// The titles to recommend to `server`'s User `user_id`, best first, by their ids in the first
    /// of `namespaces` each has, leaving out what their Account finished. Empty while they aren't
    /// Linked.
    pub(crate) async fn for_user(
        &self,
        server: &LinkedServer,
        user_id: i64,
        namespaces: &[String],
    ) -> Result<Vec<RecommendedTitle>, sqlx::Error> {
        let Some(account) = self.links.linked_account(server, user_id).await? else {
            return Ok(Vec::new());
        };
        // shortcut: an Episode's Viewing Event names the Episode, not its Show, and the Hub's titles
        // hold no Episodes, so only Movies count; count Shows once an event names its Show (#1950).
        let finished: Vec<(TitleKind, ExternalId)> = (self.viewing.list(account).await?)
            .into_iter()
            .filter(|listed| listed.event.finished && listed.event.kind == Kind::Movie)
            .filter_map(|listed| Some((TitleKind::Movie, external_id(&listed.event.external_id)?)))
            .collect();
        titles::recommended(&self.db, &finished, namespaces).await
    }
}

/// The External ID a Viewing Event names as `tmdb-603`: a namespace has no `-`, though a value may.
fn external_id(id: &str) -> Option<ExternalId> {
    let (namespace, value) = id.split_once('-')?;
    Some(ExternalId {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
    })
}
