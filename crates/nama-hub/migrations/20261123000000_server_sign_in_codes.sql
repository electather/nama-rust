-- The SHA-256 hash of the one-time code the Hub puts in a sign-in's return address once the
-- Account approves it. The Server must finish with it, so only the browser sent back there can.
ALTER TABLE server_sign_ins ADD COLUMN code_hash bytea;
