# A Transcode to download is kept until it's fetched, within a budget

A Device that can't Direct Play a Version, or wants a smaller file, such as an older iPhone and a 4K title, downloads a Transcode of it into one MP4 instead (#1422, option 2). Unlike a plan's segments, which ADR 0062 bounds by time, the file is whole and waits for its Device, which may be offline for a day: an unfetched file left on the disk is Jellyfin's transcode cache filling it (jellyfin#3929) again.

So the Server keeps each file in a folder of the segment folder until a response has sent its last byte, a whole file or a range to its end, or for 24 hours once made if none has, or until its Version is gone. One still being made 24 hours after it was asked for is stopped and deleted, as its ffmpeg has hung, so it can't hold its share of the budget for good. What the files take, made or being made, is bounded by a budget in `nama.toml`, `download_budget_mib`, 20 GiB by default: a request that would go past it is refused with 507 and why, never queued. A file being made counts as the most it can take, its Playback Limit's rate plus stereo audio for the Version's length, since a Transcode within a limit keeps its video to that rate; once made, it counts as its size. Only Playback Limits below Original can be asked for, so that most is always known.

The files are kept in memory, not the database: a restart forgets them, and its sweep deletes their folder, so a Device asks again. Each belongs to the User and Device that asked, as a Playback Plan does (ADR 0106): a Device that switches to another User isn't answered about it.

## Considered Options

- **Queue a request past the budget** until there's room: rejected for now. A refusal says at once what to do, fetch a file or wait for one to expire; a queue needs a place in it to show and still has no bound on how long a Device waits.
- **A Playback setting for the budget**, beside the segment folder's: not now, as it needs the client API to change. `nama.toml` is where an Admin who sizes the segment folder looks, and a setting can replace it later.
- **Original quality**: rejected, as it has no rate to count against the budget. A 1080p limit on a 1080p Version keeps its size at a known rate, and a Device that plays the Version as it is downloads the original file (#2195).
- **Delete once fetched with a grace period**, in case the last bytes never reached the Device: not now. A Device that lost them asks again.

## Consequences

- A Transcode to download doesn't count towards the Transcodes the Server may run at once; it runs at a low OS priority, so playback goes first.
- Deleting a file while a response sends it, on expiry or with its Version, doesn't cut that response off: the file it opened can still be read.
