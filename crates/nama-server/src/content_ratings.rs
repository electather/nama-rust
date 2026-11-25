//! The minimum age each ratings board's Content Ratings call for, whatever country a title's
//! metadata came from, and what a User's Maximum Age lets reach them.

/// The Content Ratings boards give by name, each with the age it calls for. A name two boards
/// share, such as `PG` (MPAA, BBFC and ACB), takes the strictest board's age. A rating that is
/// only a number, as FSK's are, calls for that age, so it needs no entry.
const AGES: &[(&str, u32)] = &[
    // MPAA
    ("G", 0),
    ("PG", 10),
    ("PG-13", 13),
    ("R", 17),
    ("NC-17", 18),
    // US TV
    ("TV-Y", 0),
    ("TV-Y7", 7),
    ("TV-Y7-FV", 7),
    ("TV-G", 0),
    ("TV-PG", 10),
    ("TV-14", 14),
    ("TV-MA", 17),
    // BBFC
    ("U", 0),
    ("UC", 0),
    ("12A", 12),
    ("R18", 18),
    // ACB
    ("E", 0),
    ("M", 15),
    ("MA15+", 15),
    ("R18+", 18),
    ("X18+", 18),
];

/// The age the Content Rating `rating` calls for, `None` for one this table doesn't know, which
/// counts as unrated. Case, spaces, a `Rated ` or `FSK` before it and a country before a `:`, as
/// Kodi's NFO files write it (`US:PG-13`), don't count.
pub(crate) fn minimum_age(rating: &str) -> Option<u32> {
    let rating = rating.trim().to_ascii_uppercase();
    let rating = rating
        .split_once(':')
        .map_or(rating.as_str(), |(_, rating)| rating);
    let rating = rating
        .trim_start_matches("RATED ")
        .trim_start_matches("FSK")
        .trim();
    let named = AGES.iter().find(|(name, _)| *name == rating);
    named.map_or_else(
        || rating.trim_end_matches('+').parse().ok(),
        |&(_, age)| Some(age),
    )
}

/// What a User's Maximum Age lets reach them: the default, with no age, lets every title.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MaximumAge {
    /// The highest age a title's Content Rating may call for, `None` for no Maximum Age.
    pub(crate) age: Option<u32>,
    /// Whether a title with no Content Rating, or one [`minimum_age`] doesn't know, reaches them.
    pub(crate) allow_unrated: bool,
}

impl MaximumAge {
    /// Whether it lets every title through, as having no age does.
    pub(crate) fn lets_every_title(&self) -> bool {
        self.age.is_none()
    }

    /// Whether a title whose Content Rating is `rating` reaches its User.
    pub(crate) fn allows(&self, rating: Option<&str>) -> bool {
        let Some(age) = self.age else {
            return true;
        };
        rating
            .and_then(minimum_age)
            .map_or(self.allow_unrated, |needed| needed <= age)
    }
}

#[cfg(test)]
#[path = "content_ratings_tests.rs"]
mod tests;
