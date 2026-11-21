# The Kernel narrows a Playback Plan step by step

Whether a Version is Direct Played, Remuxed or Transcoded depends on more and more things over time: the Version's Tracks, what the Device can decode, the network, the Server's encoders, the admin's settings, the User's Playback Limit and how busy the Server is. The Kernel builds a Playback Plan by running these as separate steps in a fixed order. Each step can only narrow what the plan may be, and each records why it did. A new input is a new step. The admin configures the steps through ordinary settings, and the reasons are shown to people, so a surprising Transcode can be explained.

ADR 0031 splits the steps: those for the User's choices run last and narrow only while a method is left. ADR 0034 types the reasons, and has a step that changes nothing give none.

A Device that fails to play a plan names the plan's Recipe: its method, the container it serves and the audio codec it sends, such as a Remux in fragmented MP4 sending E-AC-3. A new plan then rules out only that Recipe, so the method stays while it can play another way, such as a Remux converting the audio to AAC, and is ruled out once every Recipe has failed (#1416). A Device that names only failed methods still has each ruled out whole.

A Device also says how sure it is of what it decodes: exact, as Android's MediaCodecList lists its decoders; attested, as Apple's AVFoundation vouches; or declared, as a browser's `isTypeSupported` overstates, which is also how a Device that doesn't say is read. Nothing passes audio through to a receiver yet. When something does, it needs exact evidence, since audio passed through that the receiver can't decode plays as silence or noise.

## Considered Options

- **The client decides**: rejected. Every client would repeat the logic, and only the Server knows its encoders, its load and its admin's settings.
- **A rules language the admin writes, like Jellyfin's device profiles**: rejected. It's hard to get right, and settings cover what admins have asked for.
- **Plugins that Transcode**: rejected. Running ffmpeg is the Kernel's job (ADR 0006). If Plugins need a say, it will be an Extension Point that narrows the Playback Plan like any step but never runs a Transcode itself.
- **A ranked list of every candidate plan, which a failed one is skipped down**, as Silo's planner does: rejected (#1416). Ruling out the failed Recipe keeps the steps legible and gives the same replan.
