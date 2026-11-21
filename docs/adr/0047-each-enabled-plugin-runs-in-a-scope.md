# Each enabled Plugin runs in a scope

_Amended by [ADR 0079](0079-plugins-declare-jobs-and-the-kernel-runs-them-on-cron.md): Jobs replace the Tick, so a Job's Run is what runs up to its own limit._

Each enablement of a Plugin gets a scope: a cancellation token and the Kernel tasks started for it. A Kernel task that calls one Plugin repeatedly, such as Requests polling a Fulfiller about progress or a Connector being polled for its link, runs in that Plugin's scope, started through `ExtensionPoints::spawn_in`; retries a Plugin asks for will run there too. Disabling a Plugin, enabling it again with new settings and the Server shutting down cancel the scope and wait for its tasks before the Plugin is dropped, inside the order ADR 0028 sets: save, then cancel and join, then drop, then enable again. A Plugin enabled again gets a new scope, which the old one's cancel doesn't touch. A Plugin disabled from inside its own call or scope, including one that crashed, has its scope cancelled at once, and the join and the drop happen in the background, since waiting there would wait on itself.

A Fulfiller that can't act yet, such as Sonarr still adding a Show, answers `retry_after_seconds` (`TryAgainIn` to a Built-in) instead of waiting inside its call, and the Kernel schedules the retry in that Plugin's scope: a hand-off leaves the Request Approved and hands it off again after the wait, through the path that reads it again, so one cancelled or handed off meanwhile is left alone; telling it to stop following a Show is asked again the same way; and a delete goes on at once and asks it again to stop monitoring later, without naming it as not told. Disabling the Plugin ends a waiting retry at once. Retries are capped at 10 in a row for one Request or title, after which a hand-off fails the Request with the Plugin's last message and the others log that they gave up.

Cancelling a scope also stops what the Kernel isn't awaiting. An Installed Plugin's call runs on a blocking thread, which goes on after the Kernel stops awaiting it, so each enablement is handed a `Stop` through its `Registrar`, which the scope's cancel fires, contained, before waiting for its tasks. The Installed adapter hooks its running calls to it: a disable, a re-enable with new settings or grants, or shutdown interrupts every WASM call of the old enablement through Extism's cancel handle, and refuses any it would start after. Such a call fails plainly, as "the Plugin was disabled", and isn't counted as a crash. The old enablement's Kernel functions refuse from the cancel on, checked on each call, so nothing it asks after a disable or grant change reaches the Kernel; a function already past that check, such as a delete that started, goes on. A Built-in Plugin whose work is all futures the Kernel awaits needs no `Stop`.

## Considered Options

- **Each caller stops its own tasks**: rejected. Every caller would have to learn that a Plugin was disabled, and one that missed it would keep calling a Plugin that is gone, or hold it past its drop.
- **Aborting the tasks instead of joining them**: rejected. An aborted task may still hold the Plugin's handlers when its `Drop` runs, outside containment, and the caller can't tell when the Plugin is really gone.
- **One scope per Plugin across enablements**: rejected. A task started for the old settings could keep running against the new ones.
- **Letting a running Installed call finish on its own**: rejected. A tick runs up to its own limit with the old enablement's Kernel functions and grants, so it could start deletes after a disable or a revoked grant has returned.

## Consequences

- `Registry::disable` is async and must not be called from inside the Plugin's own call or scope; `Registry::disable_now` is for that, and its drop happens after it returns.
- A task that ignores cancellation and never yields holds up a disable, and so the Plugin settings lock, until it ends.
- Shutdown joins every scope after the crashed Plugins are saved, leaving the Plugins enabled in memory.
