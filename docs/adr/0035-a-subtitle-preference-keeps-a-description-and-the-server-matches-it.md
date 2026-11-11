# A Subtitle Preference keeps a description, and the Server matches it

The subtitle a User picked lived only in the player, so it was off again in the next Episode. Now the Kernel keeps a Subtitle Preference for each User: that they turned subtitles off, or what describes the Track they picked, which is its language, whether it is forced-only, whether it is SDH, and its title. A Track's place isn't kept, because each Episode is its own file. One is kept for each Show or Movie, by its external id as Watch State is (ADR 0013), and one for everything the User plays, set in Settings.

When a plan request names no subtitle, the Kernel takes the Preference for the Show or Movie, or else the User's, and matches it against the Version's subtitle Tracks. Language and forced must match. Among those Tracks it prefers the same SDH, then the same title, then a Track the Device shows itself over one that needs Burn-in. The Playback Plan says which subtitle playback starts with, even one the Device draws. That subtitle is still no input to how the Version plays (ADR 0033). A match that needs Burn-in is one of the User's choices, so the plan drops it when the Server can't Transcode (ADR 0031).

Only the User's own pick changes a Preference. The player reports each pick in its own call, naming the Version and the Track's place, or that subtitles are off, and the Kernel reads the description from the Tracks it stored.

## Considered Options

- **Kept per Season**: rejected. The User would pick again at every Season.
- **Kept with the Watch State**: rejected. Watch State is progress through one Movie or Episode, and this belongs to a Show.
- **The player matches**: rejected, as in ADR 0027. Every client would repeat the rule, and a match that needs Burn-in would cost two plan requests.
- **A plan request counts as a pick**: rejected. The Server never hears of a subtitle the Device draws, and the player repeats a burned-in one in later requests, such as after an audio change.
- **Keep the Track's format**: rejected. Whether a Track is drawn or burned in depends on the Device, so a pick on one Device would force a Transcode on another that has the same subtitle as text.
- **A separate store for the User's default**: rejected. A default language is the same description without a title, matched by the same rule.

## Consequences

- A plan request that names no subtitle now asks the Server to choose, so a player says outright that it wants none. It takes the plan's starting subtitle only for the first plan of a playback, and after that sends what the User has on screen.
- An Unmatched Item keeps no Preference, as it keeps no Watch State.
- A dropped Burn-in, a renderer that failed and an Episode where nothing matched all leave the Preference as it was. So does a subtitle turned on for one scene, until the User turns it off.
- A Show's Preference doesn't follow a later change to the User's. A new pick replaces it, and nothing else clears it.
- Languages are compared as languages, so `eng` matches `en`. A Track with no language matches only another with none.
- Nothing is said when no Track matches. A Playback Reason for it would be API contract for good (ADR 0034), and the subtitle menu already shows what the Version has.
- An audio Track's Preference is another kind in the same store, kept by the same call.
- The Hub gets no copy.
