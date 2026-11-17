-- Whether the User hid the title from Continue Watching (#916). Any write to the row's Watch State
-- clears it, so hiding lasts until the User plays the title again; a Show is hidden while the row
-- that dates its entry is.
ALTER TABLE watch_states ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1));
