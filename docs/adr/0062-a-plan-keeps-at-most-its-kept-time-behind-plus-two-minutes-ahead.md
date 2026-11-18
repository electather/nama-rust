# A plan keeps at most its kept time behind plus two minutes ahead

A running Remux or Transcode deletes its files ending more than "Keep for skipping back" before the last segment asked for (#671). #671 decided to delete only behind: files ahead of the last segment asked for stayed until playback passed them. But each seek back leaves ffmpeg's last stretch ahead, up to the kept time plus 2 minutes, and repeated seeks back leave one stretch each, so one plan could hold close to a whole Version, tens of GB for a 4K Remux. Jellyfin's transcode cache filling the disk (jellyfin#3929) is the trap this avoids.

So this replaces #671's "Only behind": on each tick, a running plan also deletes its files starting 2 minutes (`RUN_AHEAD_MS`) or more past the last segment asked for. While its current ffmpeg run runs, it keeps those less than 2 minutes past where that run started, since a seek ahead started it there and a player may still ask for an earlier segment. If ffmpeg may have written a file the tick deleted before the run saw it, the tick stops the run, which could no longer tell how far it has written, and the request for that file starts ffmpeg again. A plan then holds at most its kept time behind plus 2 minutes ahead, which is what an Admin sizes the segment folder by: about 1.4 GB for a 4K Remux at the default 5 minutes. Skipping ahead into a deleted stretch waits while ffmpeg writes it again, as skipping back further than the kept time already does.

## Considered Options

- **Delete only behind**, as #671 decided: rejected for the seek-back stretches above.
- **A size cap on the segment folder**, a new Admin setting that stops ffmpeg and refuses new Remuxes and Transcodes once the folder is full: not now. It is the only option that protects a small tmpfs, but it needs a setting, a Refusal and a default no number fits both a 64 MiB tmpfs and a NAS. Revisit it if a real install fills the folder.

## Consequences

- No new setting: the bound follows from "Keep for skipping back" and `RUN_AHEAD_MS`.
- The total still grows with how many plans run at once, since Remuxes have no limit.
