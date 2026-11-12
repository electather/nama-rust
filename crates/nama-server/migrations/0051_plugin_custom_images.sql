-- A Plugin's logo may have a Custom Image too, at `plugin/<id>/logo`, and it may be an SVG
-- (ADR 0036). SQLite can't change a CHECK in place, so the table is rebuilt with `image/svg+xml`
-- in `content_type`. Nothing references it, so dropping it is safe.
CREATE TABLE custom_images_new (
    -- Which image of which title or Plugin it stands in for, as its paths name it:
    -- `movie/tmdb-348/poster`, `show/tmdb-1399/backdrop`, `show/tmdb-1399/season/2/poster` or
    -- `plugin/sonarr/logo`.
    slot TEXT PRIMARY KEY,
    -- Taken from the bytes, so its URL changes whenever it does.
    version TEXT NOT NULL,
    -- Only a Plugin's logo is ever an SVG.
    content_type TEXT NOT NULL CHECK (
        content_type IN ('image/jpeg', 'image/png', 'image/webp')
        OR (content_type = 'image/svg+xml' AND slot LIKE 'plugin/%')
    )
) STRICT;

INSERT INTO custom_images_new SELECT * FROM custom_images;
DROP TABLE custom_images;
ALTER TABLE custom_images_new RENAME TO custom_images;
