# Each enabled Plugin runs in a scope

Each enablement of a Plugin gets a scope: a cancellation token and the Kernel tasks started for it. A Kernel task that calls one Plugin repeatedly, such as Requests polling a Fulfiller about progress or a Connector being polled for its link, runs in that Plugin's scope, started through `ExtensionPoints::spawn_in`; retries a Plugin asks for will run there too. Disabling a Plugin, enabling it again with new settings and the Server shutting down cancel the scope and wait for its tasks before the Plugin is dropped, inside the order ADR 0028 sets: save, then cancel and join, then drop, then enable again. A Plugin enabled again gets a new scope, which the old one's cancel doesn't touch. A Plugin disabled from inside its own call or scope, including one that crashed, has its scope cancelled at once, and the join and the drop happen in the background, since waiting there would wait on itself.

## Considered Options

- **Each caller stops its own tasks**: rejected. Every caller would have to learn that a Plugin was disabled, and one that missed it would keep calling a Plugin that is gone, or hold it past its drop.
- **Aborting the tasks instead of joining them**: rejected. An aborted task may still hold the Plugin's handlers when its `Drop` runs, outside containment, and the caller can't tell when the Plugin is really gone.
- **One scope per Plugin across enablements**: rejected. A task started for the old settings could keep running against the new ones.

## Consequences

- `Registry::disable` is async and must not be called from inside the Plugin's own call or scope; `Registry::disable_now` is for that, and its drop happens after it returns.
- A task that ignores cancellation and never yields holds up a disable, and so the Plugin settings lock, until it ends.
- Shutdown joins every scope after the crashed Plugins are saved, leaving the Plugins enabled in memory.
