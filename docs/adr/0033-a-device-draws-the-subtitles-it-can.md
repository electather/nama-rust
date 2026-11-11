# A Device draws the subtitles it can

Image and styled subtitles, such as PGS and ASS, reached the screen only by Burn-in, so turning one on Transcoded the video, and a Server that couldn't Transcode a Version couldn't show them at all. Now a Device tells the Server which subtitle formats it can draw, alongside what it can decode, and the Playback Plan lists each of the Version's subtitle Tracks once: among those the Device shows itself, or among those the Server can burn in. A subtitle the Device shows is no input to the plan, so turning it on never changes how the Version plays or restarts it. Burn-in is left for the formats a Device can't draw.

An ASS Track counts as one a Device can draw only when its file carries fonts. A renderer on the Device sees just those fonts and one fallback for Latin, Greek and Cyrillic, where the Device's own text and the Server's ffmpeg see the fonts of the system they run on.

## Considered Options

- **Burn in whenever the video Transcodes anyway**: rejected. Every subtitle switch would restart the Transcode, a Burn-in moves filtering off the GPU, and a subtitle burned into a picture scaled down for a Playback Limit is blurrier than one drawn at the screen's size. One rule is also easier to say in the plan's reasons.
- **The player decides, and doesn't ask for a Burn-in it doesn't need**: rejected, as in ADR 0027. Every client would repeat the rule, and only the Server knows whether a file carries fonts.
- **Draw every ASS Track, with a folder of fallback fonts the Admin fills**: rejected for now. Until the folder is filled, a Track in Persian, Arabic, Chinese or Japanese with no fonts of its own draws as empty boxes, where its plain WebVTT shows correctly.
- **A setting that forces Burn-in**: rejected. Nobody has asked for one, and a renderer that fails falls back by itself.

## Consequences

- A Device whose renderer fails stops saying it can draw that format until it is reloaded, and asks for a new plan, which burns the Track in or drops it (ADR 0031).
- A styled ASS Track in a file without fonts still needs a Burn-in to keep its styling.
- Which renderer draws a format is the client's business, and changes without the Server knowing.
