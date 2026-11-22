-- A Plugin's logo is always its Manifest's (#1750), so the Custom Images admins uploaded for one go.
-- Their files in the image store go at startup (`load_settings` in `src/lib.rs`), which SQL can't reach.
DELETE FROM custom_images WHERE slot LIKE 'plugin/%';
