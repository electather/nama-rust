# One Server binary with requests built in

Media serving and Request management run in a single Server process with one user model and one database. Today they are separate apps (Jellyfin/Plex plus Seerr), so Seerr has to copy users from the media server, sync them, and poll it to learn what's available. Keeping them in one binary removes that whole class of sync problems. Internally the code is still split into crates in a Cargo workspace, and the Hub is a separate binary in the same workspace.
