# Migrations start from one file per table; later ones are named by UTC timestamp

A Server migration's file name prefix is its version, which every database records once it has run. Sequential numbers made two parallel PRs pick the same next number: #546 and #555 both added `0041`, merged without a conflict and left main red. `migrations.txt` then turned that into a git conflict, but every two PRs adding a migration still conflicted on it and one of them renumbered its file and its tests. By then 79 migrations had piled up, nine of them rebuilding a table through a `_new` copy, so no single file said what a table looks like.

So, once, the 79 migrations were squashed into one migration per table, holding that table's schema as all 79 left it, with its indexes, triggers and seed rows, and no backfills. Those files are numbered `0001` onwards and never edited again. Every later migration is named by its UTC creation time, `YYYYMMDDHHMMSS_name.sql` (`sqlx migrate add --timestamp`), as drizzle-kit v1 does, so two branches can't pick the same version.

## Consequences

- A database created before the squash doesn't open: its recorded versions no longer match. Nothing converts it. The only release, v0.1.0, had five migrations and a handful of downloads.
- A migration can merge after one with a later timestamp. sqlx applies every migration a database hasn't run, in version order, so a release applies them in the same order everywhere; only a dev database that ran main in between sees them the other way round. A migration therefore assumes only what was on main when its PR opened, and is never renamed once pushed, since a database that ran it would then miss its version.
- No check that two parallel migrations commute, as drizzle-kit's `check` does: CI runs every test on a fresh database in sorted order, which is what users get.
- Squashing again (say, at a release) is a new decision, not part of this one.
