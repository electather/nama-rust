-- The default Home layout as JSON: Featured's flag and the Home rows it places, in order. Unset
-- until an Admin saves one, when Featured and every row are shown, in the order Home lists them.
ALTER TABLE server_settings ADD COLUMN default_home_layout TEXT;
