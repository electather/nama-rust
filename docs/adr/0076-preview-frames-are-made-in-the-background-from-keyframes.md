# Preview frames are made in the background from keyframes

The player's hover card shows the frame at the hovered time (#1213). The Kernel makes each Version's Preview frames ahead of time: one ffmpeg pass that decodes keyframes only (`-skip_frame nokey`), taking one frame every 10 seconds at 320 px wide, packed into JPEG sheets of 10 by 10. Decoding keyframes only is about a hundred times faster than decoding every frame, which is what made Jellyfin's trickplay take days on large libraries; a frame lands on the keyframe nearest its time rather than exactly on it.

The sheets live in the blob store (ADR 0052) under `preview-frames/`, keyed to the Version and deleted with it, and switching stores copies them. A sheet is far under `BLOB_MAX_BYTES`. The client gets the interval, frame size, sheet layout and count on the Playback Plan and fetches sheets over plain HTTP, as media bytes are (ADR 0007). A Version without them yet plays without Preview frames.

This reads each Version's whole file, which ADR 0066 avoided for listing keyframes after a Scan because it competes with playback for the disk. Preview frames can't come from a container's index, so the Kernel makes them one Version at a time, after Probing, and waits while any Playback Plan is running. Each Library has an Admin switch for them, on by default.

The work is a query for the Versions that need Preview frames, worked out from what's stored, and one call that makes them for a single Version and can run twice. A failure is stored with the file's size and modification time, as Probing's are, so a changed file is tried again. A loop on its own `TaskTracker` joins them today, so a jobs system can take over the loop alone.

## Considered Options

- **Decode every frame**: rejected. It takes far longer for a sharper frame no one scrubbing needs.
- **Seek to each keyframe the Version lists (ADR 0066)**: rejected for now. Hundreds of seeks per Version cost an ffmpeg start each, and Versions without listed keyframes would still need the whole-file pass.
- **Make them the first time a Version plays**: rejected. The first viewer, often the only one, would get none.
- **Quiet hours**: left out. Nama has no schedule for heavy work yet; pausing during playback covers the conflict that matters.

## Consequences

- After an upgrade, every Version in a Library with the switch on gets Preview frames, one by one, which keeps a disk busy for a while on a large Library.
- The blob store's copy on a switch of stores grows with Preview frames.
