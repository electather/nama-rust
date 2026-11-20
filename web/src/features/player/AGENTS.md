## web/src/features/player

- **A native `<video>` with Nama's own controls**, built from Base UI primitives. It plays the URL of the Playback Plan it asks the Server for on opening (`usePlaybackPlan`), sending what the browser says it decodes just then (`findDeviceDecoding`), which nothing stores. A "Playback info" panel shows the plan's method, Tracks and reasons.
- **Subtitles never ask for a new plan.** The plan's subtitles are ones the Device shows itself, so picking one only changes which shows. Text ones are `<track>`s that `useSubtitles` shows or hides. `useDrawnSubtitle` draws PGS and VobSub ones over the video with libbitsub, and ASS ones with the fonts their file carries with JASSUB, fetching each renderer only then.
- **What the web app says it draws** is PGS and VobSub whenever the browser has WebAssembly, and ASS when it also has Web Workers, until a format's renderer fails, when it asks for a plan that burns the Track in instead (ADR 0033).
- **JASSUB's notices ship from `web/public/licenses/jassub.txt`**, since its package carries none for the libraries it bundles. Generate it again whenever you bump `jassub` (`web/README.md`, "JASSUB's notices"). Its fallback font's OFL notice ships from `web/public/licenses/liberation-sans.txt`, since the font carries no license in its metadata.
- **A Remux or Transcode plays an HLS playlist.** `usePlaybackSource` gives it straight to the `<video>` where the browser plays HLS itself (`playsHlsNatively`), and otherwise plays it through hls.js, which it fetches only then.
- **The routes are full screen.** They set `staticData: { fullScreen: true }`, which tells the `_member` layout to leave out the app chrome.
- **A plan that can’t play shows its reasons**, and a file the browser can’t play anyway a message, not a broken player.
- **Watch State is kept in the background.** `useWatchState` resumes where you stopped and reports without playback waiting on it, retrying failures without showing them.
