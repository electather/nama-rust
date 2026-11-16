# Plugins automate through a Kernel service

In the Provider lane the Kernel asks a Plugin. The Automation lane is the other way round: a Plugin asks the Kernel, so it can act on the Server, such as deleting what every User has watched. The Kernel still starts every call. It sends Event batches to an Observer (`EventObserverService`, unchanged) and a scheduled `Tick` to a Plugin serving `AutomationService`, every `tick_every_seconds` its Manifest sets, at least 60. During those calls, and in fact during any call of the enablement, the Plugin may call the Kernel's functions: the imports in the Wasm namespace `nama` that `automation.proto` documents. They are few: a per-Plugin Store, with an optional per-User scope; reads of Libraries, titles, Watch State and the Users, each User's id and display name only, so a Plugin can tell who "every User" is when only some have a Watch State (every User sees every Library, so these are every User a title is for); and operations that already have one owner, reused rather than copied, starting with delete through `Scanner::delete_media`. Each needs a Plugin Permission, which the Manifest requests in `permissions` and the Admin grants when enabling the Plugin, and the Kernel checks it on every call.

The functions are documented in `automation.proto`, not declared as a service. They are not an Extension Point, no Manifest names them, and the WASM adapter's rows are for the exports the Kernel calls; they are versioned with `AutomationService` instead. Each takes its request message and answers a `KernelAnswer` envelope: the response, or `refused` with why in plain words, such as a Permission not granted. A refusal is a value the Plugin handles, not a failed call that would unwind it. A failed tick is logged and counted toward the crashes in a row, as an Observer's failure is, and shown to no one.

## Considered Options

- **The whole client API**: rejected, and on the design's list of what is left out. It would give a Plugin every User's powers, and every change to the client API would become a change to the Plugin contract.
- **Plugin-defined points**: rejected. A Plugin would offer something other Plugins call, and Plugins never depend on each other; the Kernel defines every Extension Point.
- **A function per feature, added as Plugins need them**: chosen. A new function joins `automation.proto` with the Permission it needs and a minor bump of `AutomationService`, and reuses the operation's one owner in the Kernel.

## Consequences

- A Plugin naming `AutomationService` must export `tick` and set `tick_every_seconds` to at least 60; the Kernel refuses it otherwise, as it refuses a Manifest requesting a Permission it doesn't know.
- An operation a Plugin may start through the Kernel keeps one owner: the function calls it, so a Plugin's delete is checked and done as an Admin's is.
- An Admin may grant fewer Permissions than a Plugin requests; the functions the missing ones cover answer `refused`, and the Plugin carries on.
- A file holding two Episodes goes with either, which an Admin's delete shows before it happens. A Plugin sees no such dialog, so `delete_media` refuses, deleting nothing, a delete that would take along an Episode `DeleteAsked.also` doesn't name, and names each one; `DeleteStarted.taken_along` says what went along, so the Plugin doesn't ask to delete it again.
- `list_titles` answers a page at a time, by kind and then id, with an opaque `next_page_token`, so a tick over a large Library needn't fit in one message; a `page_size` of 0 keeps the whole Library in one answer for a Plugin built before. The Kernel still reads the whole Library for each page.
- `delete_media` starts the Scanner's delete in a task outside the Plugin's scope (ADR 0047), since it never calls the Plugin back. Disabling the Plugin, or revoking its delete Permission, doesn't stop a delete it already started.
