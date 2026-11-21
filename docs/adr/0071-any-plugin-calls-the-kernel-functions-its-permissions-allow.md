# Any Plugin calls the Kernel functions its Permissions allow

_Amended by [ADR 0079](0079-plugins-declare-jobs-and-the-kernel-runs-them-on-cron.md): `AutomationService` and its `Tick` are gone, and Jobs replace them._

ADR 0048 gave the Kernel's functions to Plugins whose `points` name `AutomationService`, but the Kernel never checked `points`: it checks only the Plugin Permission each function needs (ADR 0049). A Plugin serving another Extension Point now needs one too. The OpenSubtitles.com Plugin gets a fresh instance on every call, so it has nowhere to keep its login token between downloads, and OpenSubtitles limits logins more tightly than other calls (#1268). We decided the granted Plugin Permission alone decides which Kernel functions any Plugin may call, during any of its calls. The written contract now says what the Kernel already does. The Admin agrees to each Permission when enabling the Plugin, so checking `points` as well would protect nothing.

## Considered Options

- **The Store for any Plugin, the other functions for Automation Plugins only**: rejected. The Kernel would need a new `points` check and a split rule, and it would protect nothing the Admin's grant doesn't already protect.
- **The Kernel functions for Automation Plugins only, enforced**: rejected. A subtitle source would have to export a `tick` it doesn't need, with `tick_every_seconds` set, just to keep a token.
- **A Kernel-held cache per Plugin, or instances that live between calls**: rejected. One would add contract surface the Store already covers. The other would give up the fresh instance, which keeps a call that traps from leaving anything behind for the next one.

## Consequences

- The Kernel functions are versioned with `CONTRACT_VERSION`, not with `AutomationService`, which now covers only `Tick`.
- A Plugin's Manifest may request Plugin Permissions whatever its `points`, and the Plugins page asks the Admin to grant them as it does for an Automation Plugin.
- A Plugin should carry on when a function answers `refused`. A subtitle source not granted Store logs in again for each download.
