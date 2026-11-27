//! A User's Reach: their Library access and their Maximum Age together, the one answer to what
//! they may see (`AGENTS.md`).

use std::sync::Arc;

use super::{
    Users,
    library_access::{LibraryAccess, library_access},
    maximum_age::maximum_age,
    until_heard,
};
use crate::{content_ratings::MaximumAge, db::Database};

/// How many changes to Users' Library access or Maximum Age may happen before a waiter in
/// [`Users::reach_watched`] has seen them. One that falls further behind reads its User's Reach
/// instead.
pub(super) const REACH_CHANGES_BUFFERED: usize = 64;

/// What a User may see: the Libraries they see, and the titles their Maximum Age lets through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reach {
    pub(super) libraries: LibraryAccess,
    pub(super) max_age: MaximumAge,
}

impl Reach {
    /// Every Library and every title, for system work such as sync, never a User's default.
    pub(crate) const fn everything() -> Self {
        Self {
            libraries: LibraryAccess::All,
            max_age: MaximumAge {
                age: None,
                allow_unrated: true,
            },
        }
    }

    /// Whether they see the Library `library`.
    pub(crate) fn sees(&self, library: i64) -> bool {
        self.libraries.sees(library)
    }

    /// Whether a title of the Library `library` rated `rating` reaches them.
    pub(crate) fn reaches(&self, library: i64, rating: Option<&str>) -> bool {
        self.sees(library) && self.allows_rating(rating)
    }

    /// Whether an Unmatched Item of the Library `library` reaches them: it counts as unrated.
    pub(crate) fn reaches_unmatched(&self, library: i64) -> bool {
        self.reaches(library, None)
    }

    /// Whether their Maximum Age lets a title rated `rating` through, wherever it is: for a title
    /// in no Library, such as a missing one, or a Request's.
    pub(crate) fn allows_rating(&self, rating: Option<&str>) -> bool {
        self.max_age.allows(rating)
    }

    /// Whether a title with no known Content Rating reaches them.
    pub(crate) fn allows_unrated(&self) -> bool {
        self.allows_rating(None)
    }

    /// Whether every Library and every title reaches them, so no title needs judging.
    pub(crate) fn lets_everything(&self) -> bool {
        self.libraries == LibraryAccess::All && self.lets_every_rating()
    }

    /// Whether their Maximum Age lets every title through, whichever Libraries they see.
    pub(crate) fn lets_every_rating(&self) -> bool {
        self.max_age.lets_every_title()
    }

    /// Those of the stored Content Ratings `stored` that reach them, with the empty string for
    /// none if unrated titles do, for SQL paging to bind: `None` for every one.
    pub(crate) fn kept_ratings(&self, mut stored: Vec<String>) -> Option<Vec<String>> {
        if self.lets_every_rating() {
            return None;
        }
        stored.retain(|rating| self.allows_rating(Some(rating)));
        if self.allows_unrated() {
            stored.push(String::new());
        }
        Some(stored)
    }
}

/// The Reach of the User `user`, read once per request: none of a User who isn't there.
pub(crate) async fn reach(db: &Database, user: i64) -> Result<Reach, sqlx::Error> {
    Ok(Reach {
        libraries: library_access(db, user).await?,
        max_age: maximum_age(db, user).await?,
    })
}

impl Users {
    /// The Reach of the User `user` now, and a future that resolves once their Library access or
    /// Maximum Age is set again, so whatever they opened under it can end, to be opened again
    /// under the new one. It resolves at once if theirs no longer reads the same, or can't be read.
    pub(crate) async fn reach_watched(
        &self,
        user: i64,
    ) -> Result<(Reach, impl Future<Output = ()> + Send + use<>), sqlx::Error> {
        // Listening before reading means no change falls between the two.
        let changes = self.reach_changes.subscribe();
        let seen = reach(&self.db, user).await?;
        let (db, held) = (Arc::clone(&self.db), seen.clone());
        let set_again = until_heard(changes, user, async move || {
            let read = reach(&db, user).await;
            read.inspect_err(|err| tracing::error!("could not look up a User's Reach: {err}"))
                .is_ok_and(|read| read == held)
        });
        Ok((seen, set_again))
    }

    /// Tells [`Self::reach_watched`]'s waiters for the User `user` that their Reach was set again.
    pub(super) fn tell_reach_changed(&self, user: i64) -> impl FnOnce() + Send + 'static {
        let changes = self.reach_changes.clone();
        move || {
            // Fails only when nobody is waiting, which is fine.
            let _: Result<usize, _> = changes.send(user);
        }
    }
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
