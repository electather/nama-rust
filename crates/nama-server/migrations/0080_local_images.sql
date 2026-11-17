-- The images a scan found beside each Movie's file, by the names Jellyfin and Kodi give them, which
-- stand in for its downloaded Poster and Backdrop (#732). Each is a copy in the image store, under
-- `local-images/<library>/<stored>`; the Server never writes the file. Rows go with their Version.
CREATE TABLE local_images (
    version_id INTEGER NOT NULL REFERENCES library_files (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('poster', 'backdrop')),
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_ns INTEGER NOT NULL,
    -- The version the image store took from its bytes.
    stored TEXT NOT NULL,
    content_type TEXT NOT NULL,
    PRIMARY KEY (version_id, role)
) STRICT;
