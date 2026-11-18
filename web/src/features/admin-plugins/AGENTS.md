## web/src/features/admin-plugins

- **Plugin logos only as images.** A Plugin's `logoUrl` may be SVG an Installed Plugin's author or an admin wrote. Show it only through `PluginLogo`, as an `<img>` from that URL, never fetched and put inline, where its scripts, styles and external loads would reach the page.
- **Plugins aren't sandboxed.** Copy may say "an Installed Plugin can only call what you granted", never "sandboxed" (ADR 0049).
