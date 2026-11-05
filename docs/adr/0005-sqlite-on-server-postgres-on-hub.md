# SQLite on the Server, PostgreSQL on the Hub

A Server uses SQLite (WAL mode, a single writer, short transactions during scans) so a self-hosted install is one binary and one file with nothing else to run. The Hub is a multi-tenant service holding many accounts and their Watch State, so it uses PostgreSQL. Each binary supports exactly one database, so migrations are written for one dialect only.
