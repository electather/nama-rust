# An upgrade migrates all at once, forward only, with foreign keys off

sqlx runs each migration in its own transaction on a connection with foreign keys on. So an upgrade that fails at its third migration leaves a database two migrations ahead, which neither the new Server nor the old one opens. And a migration that rebuilds a table through a `_new` copy can't turn foreign keys off, since SQLite ignores `PRAGMA foreign_keys` inside a transaction: dropping a table others reference with `ON DELETE CASCADE` would delete their rows. drizzle-kit generates exactly that rebuild and has lost users' data to it (drizzle-team/drizzle-orm#5782, #4640, #4089). Until now, rebuilds only avoided referenced tables, by a comment in each.

So the Server applies every pending migration inside one transaction, with foreign keys off. Before committing, it runs `PRAGMA foreign_key_check` and rolls the whole upgrade back if any row breaks a key. Then it turns foreign keys back on. Migrations only go forward: there are no down migrations, and the way back from an upgrade is the copy of the database taken before it (`nama.db.before-upgrade`).

## Considered Options

- **Down migrations** (sqlx's `.down.sql`): rejected. One can't bring back what its up migration dropped, undoing a rebuild is another rebuild that nothing exercises, and the copy taken before the upgrade already restores the exact database. Drizzle declined them too.
- **One transaction per migration**, as sqlx does: rejected for the half-upgraded database above.
- **Rebuild only tables nothing references**: rejected. It's kept by a comment, not a check, and it rules out changing a CHECK on `users` or `libraries`, which most tables reference.

## Consequences

- No migration may be `-- no-transaction`, so none can `VACUUM` or change the journal mode.
- A failed upgrade changes nothing, so the version it upgraded from still opens the database.
- A rebuild may drop a referenced table, but it must recreate the table's indexes and triggers itself: `DROP TABLE` takes them along.
