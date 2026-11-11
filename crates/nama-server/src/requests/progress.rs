//! Download progress: what each Request's Fulfiller says it is downloading for it, asked while the
//! Fulfiller has any Request that isn't Available yet, kept in memory only and attached to a
//! Request read for a viewer (ADR 0012).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use nama_plugin_api::fulfiller::{Download, Fulfiller, Movies, Shows};
use nama_proto::nama::v1::{Request, RequestProgress, RequestState};
use sqlx::SqliteConnection;
use tokio::{sync::Notify, time::error::Elapsed};
use tracing::{error, warn};

use super::{load, lock, stored};
use crate::{
    db::Database,
    events::{EventBus, event},
    plugins::Registry,
};

/// The states of a Request its Fulfiller has that isn't Available yet. Its Fulfiller has it once
/// it stored a reference, which only the read in [`fulfilling`] sees.
const FULFILLING: [RequestState; 2] = [RequestState::Processing, RequestState::PartiallyAvailable];

/// Asks the Fulfillers how the Requests they have are progressing, and keeps their answers.
#[derive(Debug, Clone)]
pub(super) struct Progress {
    db: Arc<Database>,
    events: EventBus,
    plugins: Registry,
    /// What each Request's Fulfiller said it is downloading for it when last asked, by Request id.
    kept: Arc<Mutex<HashMap<i64, Vec<Download>>>>,
    /// Wakes [`Progress::follow`] when a Request becomes Processing.
    became_processing: Arc<Notify>,
}

impl Progress {
    /// Asks about the Requests in `db` of the Fulfillers in `plugins`, publishing on `events`.
    pub(super) fn new(db: Arc<Database>, events: EventBus, plugins: Registry) -> Self {
        Self {
            db,
            events,
            plugins,
            kept: Arc::default(),
            became_processing: Arc::default(),
        }
    }

    /// What wakes [`Self::follow`] at once, for the Hand-off to notify when a Request becomes
    /// Processing.
    pub(super) fn became_processing(&self) -> Arc<Notify> {
        Arc::clone(&self.became_processing)
    }

    /// Asks the Fulfillers how the Requests they have are progressing, every `every` while any of
    /// those isn't Available yet, and at once when one becomes Processing. Runs until dropped.
    #[expect(
        clippy::infinite_loop,
        reason = "it follows progress for as long as the Server serves, which aborts it"
    )]
    pub(super) async fn follow(&self, every: Duration) {
        loop {
            let fulfilling = self.poll(every).await.unwrap_or_else(|err| {
                error!("reading which Requests the Fulfillers have failed: {err}");
                true
            });
            // A Request that became Processing while the Fulfillers were asked left a permit, so
            // this wakes at once for it.
            let woken = self.became_processing.notified();
            if fulfilling {
                let _: Result<(), Elapsed> = tokio::time::timeout(every, woken).await;
            } else {
                woken.await;
            }
        }
    }

    /// Asks each enabled Fulfiller once, waiting `every` at most, what it is downloading for the
    /// Requests it has that aren't Available yet, keeps the answers in place of the last ones, and
    /// publishes each Request whose downloads changed. Returns whether there are any such Requests.
    pub(super) async fn poll(&self, every: Duration) -> Result<bool, sqlx::Error> {
        let fulfilling = fulfilling(&mut *self.db.reader().acquire().await?).await?;
        let (movies, seasons): (Vec<_>, Vec<_>) =
            fulfilling.into_iter().partition(|request| request.is_movie);
        let any = !(movies.is_empty() && seasons.is_empty());
        let mut polled = self.ask::<Movies>(movies, every).await;
        polled.extend(self.ask::<Shows>(seasons, every).await);
        let mut changed = self.changed(&polled);
        if changed.is_empty() {
            *lock(&self.kept) = polled;
            return Ok(any);
        }
        // Read before keeping `polled`, so if this fails the next poll sees the change again and
        // publishes it then. ponytail: reads every Request to publish the changed ones; load them
        // by id if there get many.
        let all = load(&mut *self.db.reader().acquire().await?, None, None).await?;
        *lock(&self.kept) = polled;
        let events = all
            .into_iter()
            // One that became Available or Failed since it was asked about has no progress any more.
            .filter(is_fulfilling)
            .filter_map(|mut request| {
                request.downloads = changed.remove(&request.id)?;
                Some(event(RequestProgress {
                    request: request.into(),
                    ..Default::default()
                }))
            })
            .collect();
        self.events.publish(events);
        Ok(any)
    }

    /// `request` with what its Fulfiller said it is downloading when last asked, if it is one its
    /// Fulfiller has that isn't Available yet.
    pub(super) fn attach(&self, mut request: Request) -> Request {
        if is_fulfilling(&request) {
            let kept = lock(&self.kept).get(&request.id).cloned();
            request.downloads = kept.unwrap_or_default();
        }
        request
    }

    /// What `Kind`'s Fulfiller, if one is enabled, is downloading for each of `fulfilling`, by
    /// Request id, asked in one batch, waiting `every` at most. Each has nothing downloading if it
    /// couldn't say.
    async fn ask<Kind: 'static>(
        &self,
        fulfilling: Vec<Fulfilling>,
        every: Duration,
    ) -> HashMap<i64, Vec<Download>> {
        if fulfilling.is_empty() {
            return HashMap::new();
        }
        let references: Vec<String> = fulfilling
            .iter()
            .map(|request| request.reference.clone())
            .collect();
        let references = references.as_slice();
        let answered = self
            .plugins
            .call_provider_blanking_secrets::<dyn Fulfiller<Kind>, _, _>(|fulfiller| async move {
                // Bounded, so a Fulfiller that never answers can't stop every later poll. Inside
                // the call, so formatting and dropping the Plugin's error are contained.
                let asked = tokio::time::timeout(every, fulfiller.progress(references));
                match asked.await {
                    Ok(answered) => answered.map_err(|error| error.to_string()),
                    Err(_) => Err("it didn't answer in time".to_owned()),
                }
            })
            .await;
        let by_reference = match answered {
            Some(Ok(by_reference)) => by_reference,
            Some(Err(reason)) => {
                warn!(
                    reason,
                    "a Fulfiller couldn't say how its Requests are progressing"
                );
                HashMap::new()
            }
            None => HashMap::new(),
        };
        fulfilling
            .into_iter()
            .map(|request| {
                let downloads = by_reference.get(&request.reference).cloned();
                (request.id, downloads.unwrap_or_default())
            })
            .collect()
    }

    /// Those of `polled`, what each Request its Fulfiller has is downloading now, that differ from
    /// what was kept.
    fn changed(&self, polled: &HashMap<i64, Vec<Download>>) -> HashMap<i64, Vec<Download>> {
        let kept = lock(&self.kept);
        polled
            .iter()
            .filter(|&(id, now)| kept.get(id).map_or(&[][..], Vec::as_slice) != now.as_slice())
            .map(|(&id, now)| (id, now.clone()))
            .collect()
    }
}

/// Whether `request` is in one of the [`FULFILLING`] states.
fn is_fulfilling(request: &Request) -> bool {
    FULFILLING.iter().any(|&state| request.state == state)
}

/// A Request its Fulfiller has that isn't Available yet, as [`Progress::poll`] asks its Fulfiller
/// about it.
struct Fulfilling {
    id: i64,
    /// Whether it asks for a Movie, rather than a Season.
    is_movie: bool,
    /// What its Fulfiller gave when it took it.
    reference: String,
}

async fn fulfilling(conn: &mut SqliteConnection) -> Result<Vec<Fulfilling>, sqlx::Error> {
    let [processing, partially_available] = FULFILLING.map(stored);
    sqlx::query_as!(
        Fulfilling,
        r#"SELECT id AS "id!", season IS NULL AS "is_movie!: bool",
                  fulfiller_reference AS "reference!"
           FROM requests
           WHERE state IN (?, ?) AND fulfiller_reference IS NOT NULL"#,
        processing,
        partially_available
    )
    .fetch_all(&mut *conn)
    .await
}
