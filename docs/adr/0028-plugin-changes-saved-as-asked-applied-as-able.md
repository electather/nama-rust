# A Plugin change is saved as asked and applied as able

What is stored for a Plugin is what the admin asked for, so a Plugin runs only while it is stored as enabled, but one stored as enabled may not be running: its settings failed to enable it, at startup or after a change, or it crashed and that isn't saved yet. Turning a Plugin on is applied first and saved once it runs, so a refused enable saves nothing. Turning one off, or changing its settings, is saved first and then applied, so a restart tries again. One module makes every such change, under one lock, and keeps this order.

## Considered Options

- **Store only what is running**: rejected. A settings change that fails to enable, such as TMDB without its key, would save it as disabled, and a restart wouldn't retry it once the key is fixed.
- **Apply first, then save, for every change**: rejected. A Plugin that won't enable with its new settings would lose them, and the admin would have to type them again.

## Consequences

- The Plugins page reports a Plugin as enabled only while it runs, so it can show disabled while the database says enabled. Nothing shows that difference yet.
- Requests learns a Plugin came up from a queue the module fills whenever it enables one, startup included, rather than the module calling Requests. That keeps the module free of Requests, which `serve` builds only after startup has enabled the Plugins, and it means enabling no longer waits for the waiting Requests to be handed off.
