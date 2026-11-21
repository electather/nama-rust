# Similar titles are merged one way again

_Supersedes [ADR 0085](0085-similar-titles-are-merged-both-ways.md)._

ADR 0085 counted, for each title, every stored Similar source answer that named it, because a title was asked about only on its first Match or a metadata version bump (ADR 0040), so older titles would never list newer ones. Upkeep now refreshes every Matched title's metadata weekly (#1609), and that refresh asks the Similar sources again. So without merging both ways a title lists one added after it within a week. That no longer pays for TMDB's rows changing visibly and for a second index on stored answers, so the merge is ADR 0084's again: only the answers about the title itself.

## Considered Options

- **Keep merging both ways, giving its reason as "up to a week" instead of "never"**: rejected. A week's delay is not worth changing every TMDB row and the extra lookup.
- **Merge one way, relying on the weekly refresh**: chosen.

## Consequences

- A title added today appears in older titles' Similar titles after the next weekly refresh, not at once.
- Titles Matched in the same batch, such as on a new Library's first Scan, are asked about before any of them is stored, so a local Similar source (#1587) finds few of them until the next weekly refresh.
- Turning off or rescheduling Upkeep's `refresh-metadata` Job delays both.
