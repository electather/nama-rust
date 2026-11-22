## Titles

Films and series from Wikidata's weekly dump, and the External ID crosswalk between them (ADR 0095).

- **Wikidata's CC0 data only.** Nothing here comes from TMDB or TVDB, nor is fetched from their APIs: their terms forbid a Hub storing or serving it (ADR 0094). A TMDB or TVDB id is stored only as Wikidata states it.
- **No Account owns a title.** `titles` and `title_ids` have no `account_id` (ADR 0097).
- **One owner.** Only this area reads or writes `titles` and `title_ids`, tests included.
- **Titles come in through `import`**, which the `import-wikidata` subcommand runs as its own process, never the serving one (ADR 0096). It replaces them in one transaction, and a dump cut short changes nothing.
- **Answers are External IDs.** A Server asks by any id it has, with a kind, and gets the title's ids in the Namespaces it uses (ADR 0042).

### Neighbours

- Uses: nothing but the database.
- Used by: `main` (`import`).
