# Similar titles are merged both ways

_Superseded by [ADR 0086](0086-similar-titles-are-merged-one-way-again.md)._

_Amends [ADR 0084](0084-similar-titles-come-from-similar-sources-stored-per-source-per-title.md)._

A Similar source is asked about a title only when its Matches are refreshed, and ADR 0040 refreshes a title only on its first Match or when the metadata version is bumped. So a title matched last year would never list one added today, and titles Matched in the same batch, asked before any of them is stored, never list each other. When the Kernel reads a title's Similar titles it now also counts every stored answer, from any source, that names this title, with that answer's weight. The rest of ADR 0084's merge is unchanged: weights clamped to 0.0–1.0, the highest weight kept for a title suggested more than once, ties to the lower Plugin id, only titles in the reading User's Libraries, the first 20.

## Considered Options

- **Accept that answers are fixed at the asked title's refresh**: rejected. A local source based on genres and Roles (#1587) would leave older titles' rows without anything added after them, and a new Library's first Scan would give nearly every title an empty row.
- **A Kernel function a Plugin's Job calls to have the Kernel ask it again for every title**: rejected. It costs one call per title per run, each starting from nothing, to learn what the stored answers already say the other way round.
- **Both ways only for sources that declare their answers symmetric**: rejected. It adds a contract field, and TMDB's recommendations read the other way round are still good suggestions.
- **Both ways for every source**: chosen.

## Consequences

- TMDB's rows change visibly: a title also shows the titles whose recommendations named it.
- A Similar source's answer about one title adds to other titles' rows. Disabling it still drops all of it, and a failed call still keeps its previous answer.
- Reading Similar titles looks up stored answers by the title they name as well as by the title asked about, so that lookup needs an index.
