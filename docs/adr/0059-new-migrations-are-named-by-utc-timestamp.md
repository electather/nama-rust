# New migrations are named by UTC timestamp

A Server migration's file name prefix is its version, which every database records once it has run. Sequential numbers made two parallel PRs pick the same next number: #546 and #555 both added `0041`, merged without a conflict and left main red. `migrations.txt` then turned that into a git conflict, but every two PRs adding a migration still conflicted on it and one of them renumbered its file and its tests. So a new migration is named by its UTC creation time, `YYYYMMDDHHMMSS_name.sql` (`sqlx migrate add --timestamp`), as drizzle-kit v1 does, and two branches can't pick the same version. `0001` to `0079` keep their numbers: renaming one changes the version existing databases recorded, and they sort before any timestamp anyway.

## Consequences

- A migration can merge after one with a later timestamp. sqlx applies every migration a database hasn't run, in version order, so a release applies them in the same order everywhere; only a dev database that ran main in between sees them the other way round. A migration therefore assumes only what was on main when its PR opened, and is never renamed once pushed, since a database that ran it would then miss its version.
- No check that two parallel migrations commute, as drizzle-kit's `check` does: CI runs every test on a fresh database in sorted order, which is what users get.
