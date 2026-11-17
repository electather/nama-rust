-- The thumbhash of each stored image resized so far: a blurred preview clients show while it loads,
-- taken from the image while its first resize has it decoded (ADR 0039). Deleted with the image.
CREATE TABLE image_thumbhashes (
    -- The image's key in the store, each part escaped as its file name, joined by `/`.
    key TEXT PRIMARY KEY NOT NULL,
    thumbhash BLOB NOT NULL
) STRICT;
