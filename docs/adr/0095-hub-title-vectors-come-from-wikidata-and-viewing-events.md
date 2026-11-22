# Hub title vectors come from Wikidata and Viewing Events, searched with pgvector

The Hub learns a vector for each title from two sources, neither of which carries a licence condition. One is the title's facts in Wikidata: cast, director, writers, genre, subject, awards, franchise and country. The other is how Accounts watch it: Viewing Events and pooled pair counts (ADR 0093). The Wikidata facts give a new title a usable vector before anyone has watched it, and behaviour outweighs them as Viewing Events build up. An Account's taste vector is weighted from the titles it finished. Similarity search runs in PostgreSQL with pgvector, the database the Hub already has (ADR 0005).

The Hub loads Wikidata's weekly dump of films and series. Wikidata's structured data is CC0, so training on it, storing it and serving results from it carry no conditions. Wikidata is also the Hub's ID crosswalk: a Server that knows a title by IMDb ID and another that knows it by TMDB ID reach the same title, and the Hub answers each in the External IDs it uses (ADR 0042, 0094).

## Considered Options

- **Embedding TMDB overviews, genres and cast**: rejected. TMDB's terms forbid using its content "in connection with" any machine-learning application.
- **Text embeddings of Wikipedia plots and TVmaze summaries**: deferred. Both are CC BY-SA, and whether share-alike reaches a vector is unsettled. They would also put an embedding model in the Hub. They come back with their own ADR if Wikidata and behaviour leave a measured gap.
- **Training on MovieLens ratings**: rejected for what the Hub ships, because its licence is for research. It is used only offline, in development, to tune and evaluate the recommender, unless GroupLens grants permission in writing.
- **IMDb datasets, OMDb, Trakt, Simkl, AniList**: rejected. Their terms allow personal use only, forbid building a database from them, or forbid bulk collection.
- **Redis for vector search**: rejected. It's a second stateful store to run, back up and keep consistent with Postgres. Nothing else needs it yet: no cache, no rate limiting across replicas, no queue.

## Consequences

- Wikidata is thin on obscure titles, episodes and anime. Those depend on behaviour alone, and the Server's own Similar sources cover them until then (ADR 0084).
- Vectors record which training run made them, the way a Match records its metadata version (ADR 0040), so a retrained model replaces them cleanly.
