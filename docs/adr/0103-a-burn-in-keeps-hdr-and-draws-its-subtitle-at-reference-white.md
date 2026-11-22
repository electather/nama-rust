# A Burn-in keeps HDR and draws its subtitle at reference white

A Transcode for a Device that shows HDR keeps HDR (#1415, #1853). Burning a subtitle into that picture used to go wrong in two ways (#1901). ffmpeg's `overlay` drops to 8 bits by default, so the picture bands. And a subtitle drawn at its SDR values lands at the top of PQ or HLG, around 10,000 nits. Jellyfin and Silo avoid both by never keeping HDR in a Transcode. Nama keeps it. The subtitle is mapped to BT.2408 reference white before it's laid over the picture: 203 nits in PQ, 75% signal in HLG on a 1000-nit display. It's laid over with `overlay=…:format=yuv420p10`. An image subtitle is mapped with `lutrgb`, which upstream ffmpeg has. A text subtitle needs libass to draw onto its own transparent layer first, which only jellyfin-ffmpeg's `alphasrc` and `subtitles=…:alpha=1` do. On an ffmpeg without them, a text Burn-in on an HDR Device tone-maps to SDR (`HDR_TONE_MAPPED_FOR_BURN_IN`), or, with no tone-mapper either, is drawn as before.

## Considered Options

- **Tone-map to SDR whenever something is burned in**, as Jellyfin and Silo do: rejected. Turning on a subtitle the Device can't draw would cost the viewer HDR. Measured at 4K on VideoToolbox, keeping HDR costs about 2% for an image subtitle and about 18% for text.
- **Only the 10-bit overlay**: rejected. The banding goes, but subtitles still glare.
- **zscale for the subtitle layer**, which would also convert BT.709 primaries to BT.2020: later. The image path would then need zimg, and most subtitles are white.
- **A setting for the subtitle's brightness**: rejected for now. ADR 0070 already keeps Subtitle Appearance away from Burn-in, and nobody has found 203 nits wrong.

## Consequences

- A coloured subtitle comes out more saturated than its author meant, since its BT.709 colours are read as BT.2020. White and grey are exact.
- The subtitle layer must be resized and converted to YUV in one `scale`. A second pass over a 4K frame cost about a third of the throughput in the experiment.
