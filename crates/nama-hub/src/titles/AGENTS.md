## Titles

Films and series from Wikidata's weekly dump, the External ID crosswalk between them, and the title vectors that find which are alike (ADR 0095).

- **Wikidata's CC0 data only.** Nothing here comes from TMDB or TVDB, nor is fetched from their APIs: their terms forbid a Hub storing or serving it (ADR 0094). A TMDB or TVDB id is stored only as Wikidata states it.
- **No Account owns a title.** No table here has an `account_id` (ADR 0097).
- **One owner.** Only this area reads or writes `titles`, `title_ids`, `title_facts`, `training_runs` and `title_vectors`, tests included.
- **Titles come in through `import`**, which the `import-wikidata` subcommand runs as its own process, never the serving one (ADR 0096). It replaces them in one transaction, and a dump cut short changes nothing.
- **Answers are External IDs.** A Server asks by any id it has, with a kind, and gets the title's ids in the Namespaces it uses (ADR 0042).
- **Vectors come from a training run**, which `train` makes as the `train-vectors` subcommand, its own process. It replaces every vector in one transaction, and each records its run (ADR 0095). Vectors move to `recommendations` once Viewing Events train them too (#1825).

### Neighbours

- Uses: nothing but the database.
- Used by: `main` (`import`, `train`), `api` (`similar`), `recommendations` (`recommended`), and the `nama-hub-eval` crate (`train`, `similar`). `crosswalk` has no caller yet.
