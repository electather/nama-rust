# nama-hub-eval

Scores the Hub's Similar titles against [MovieLens](https://grouplens.org/datasets/movielens/) ratings, so a change to the title vectors can be compared with what was there before, ahead of Nama having Viewing Events of its own. MovieLens is licensed for research, so it is used only here, in development: the harness downloads it each run, never commits it or puts it in the Hub's image, and never writes it to the database. Nothing the Hub serves is trained on it (ADR 0095).

It needs a throwaway PostgreSQL that Wikidata's dump was imported into, such as the local one in [`nama-hub`'s README](../nama-hub/README.md) once the Hub has migrated it, and `curl` and `unzip`:

```sh
curl -fsS https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz \
  | gzip -d | cargo run --release -p nama-hub -- import-wikidata
cargo run --release -p nama-hub-eval
```

Import once; after each change, run only the harness. It trains the title vectors with the Hub's code as it stands, replacing that database's, downloads the small MovieLens dataset, holds out the latest fifth of each user's ratings, recommends the titles most like those they rated 4 stars or more before that, summing each one's Similar answer, and prints Recall@10 and NDCG@10 over the held-out titles they rated 4 or more. Recall@10 divides by at most 10 held-out titles, so a perfect top 10 scores 1. A MovieLens film Wikidata lacks gets no answer, so it scores as a miss.
