# A reason for converted audio names the codec it's converted to

_Amends ADR 0034._

ADR 0034 left what a conversion makes to the plan's Tracks, and gave reasons only the values that ruled something out or changed what plays. Since #1364, a Remux or Transcode converts surround audio to E-AC-3, or else AC-3, when the Device decodes it, and to AAC otherwise. Which one is chosen is part of what plays, and it's what a User looking at a 7.1 TrueHD Track that came out as 5.1 wants to see next to the channels. So the three reasons that say audio is converted to fewer channels (`DEVICE_PLAYS_FEWER_CHANNELS`, `DEVICE_CHANNELS_UNKNOWN`, `CONVERSION_CHANNELS_CAPPED`) carry a `codec` param beside `channels`: the codec as ffprobe names it, `eac3`, `ac3` or `aac`.

## Considered Options

- **Leave the codec to the Tracks**, as ADR 0034 has it: rejected. The reason would say "converted to 5.1" while the Track beside it says E-AC-3, and a client wording the reason couldn't name both in one sentence.
- **A new kind for a codec conversion**: rejected. It would give a reason when nothing is ruled out or dropped, which ADR 0034 rejects, and a 5.1 Track converted at 5.1 already shows its codec in the Tracks.

## Consequences

- `codec` on those three kinds is API contract. Adding it broke no client, which reads params by name.
- Audio converted without dropping channels still gets no reason; the Tracks show what it plays as.
