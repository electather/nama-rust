# The Kernel narrows a Playback Plan step by step

Whether a Version is Direct Played, Remuxed or Transcoded depends on more and more things over time: the Version's Tracks, what the Device can decode, the network, the Server's encoders, the admin's settings, the User's Playback Limit and how busy the Server is. The Kernel builds a Playback Plan by running these as separate steps in a fixed order. Each step can only narrow what the plan may be, and each records why it did. A new input is a new step. The admin configures the steps through ordinary settings, and the reasons are shown to people, so a surprising Transcode can be explained.

ADR 0031 splits the steps: those for the User's choices run last and narrow only while a method is left.

## Considered Options

- **The client decides**: rejected. Every client would repeat the logic, and only the Server knows its encoders, its load and its admin's settings.
- **A rules language the admin writes, like Jellyfin's device profiles**: rejected. It's hard to get right, and settings cover what admins have asked for.
- **Plugins that Transcode**: rejected. Running ffmpeg is the Kernel's job (ADR 0006). If Plugins need a say, it will be an Intercept on the Playback Plan, which can narrow it like any step but never run a Transcode itself.
