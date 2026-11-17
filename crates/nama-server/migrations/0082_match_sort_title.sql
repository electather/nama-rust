-- The title a Library's list sorts a Match under: its Show's for an Episode, its own for a Movie,
-- lowercased and without accents or a leading English article (ADR 0055). Folding Unicode needs
-- Rust, which a migration can't run, so the Server fills the NULLs this leaves when it starts.
ALTER TABLE item_matches ADD COLUMN sort_title TEXT;
