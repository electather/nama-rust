-- An image in a Season's folder, or a Show's `season01-poster.jpg`, stands in for the Season's
-- Poster, recorded beside each of its Episodes' Versions (#1016). SQLite can't change a CHECK in
-- place, so the table is rebuilt; it has no indexes or triggers to recreate.
CREATE TABLE local_images_new (
    version_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    -- 'poster' and 'backdrop' are the Movie's, or an Episode's Show's.
    role TEXT NOT NULL CHECK (role IN ('poster', 'backdrop', 'season_poster')),
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- The version the image store took from its bytes.
    stored TEXT NOT NULL,
    content_type TEXT NOT NULL,
    PRIMARY KEY (version_id, role)
) STRICT;

INSERT INTO local_images_new SELECT * FROM local_images;
DROP TABLE local_images;
ALTER TABLE local_images_new RENAME TO local_images;
