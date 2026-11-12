# A Match remembers the metadata version that filled it

A Match stores its title's metadata once, when Matching decides it, and an automatic Match is never decided again (ADR 0023). So when the Server learns to store something new from the Provider, such as a Still, a Title Logo, genres or a Content Rating, the Matches already made never get it. Each Match therefore records the version of what the Server stores from the Provider. A Scan fetches the same title again, by the ids the Match already has, when that version is older than the current one, and stores what it gets. It never searches and never changes which title an item Matches, so automatic Matches and pins stay as they are. Any change to what a Match stores from the Provider bumps the version.

## Considered Options

- **A one-off migration per new field that clears Matches so they are decided again**: rejected. Deciding again could change a title people are already watching (ADR 0023), and each field would need its own hand-written migration.
- **Fetching what's missing when someone views the title**: rejected. The first view waits on the Provider and fails while it is unreachable, and a missing field can't tell "never asked" from "the Provider has none".

## Consequences

- Bumping the version costs, on each Library's next Scan, the fetches a first Match makes for every title in it, so Provider traffic spikes once, as it did when Matches first downloaded their images (#723).
- A Match at the current version that lacks a field means the Provider has none for it, so nothing asks again until the version goes up.
- A fetch that fails leaves the Match at its older version with what it had, and a later Scan tries again.
- A refreshed Match takes the Provider's current text as well, such as an overview it has since edited, not only the new fields.
