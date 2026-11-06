## nama-server

- **One database writer.** Every write goes through `Database::write` in `src/db.rs`. Never open a second writable SQLite connection. Long jobs such as a scan write in small batches.
- **Query data.** After changing a `sqlx::query!` or anything in `migrations/`, regenerate `.sqlx/` at the workspace root and commit it. See `README.md` for the commands.
