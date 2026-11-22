## web/src/features/admin-plugins

- **Plugin logos only as images.** A Plugin's `logoUrl` may be SVG an Installed Plugin's author wrote. Show it only through `PluginLogo`, as an `<img>` from that URL, never fetched and put inline, where its scripts, styles and external loads would reach the page.
- **Plugin setup steps only as data.** A Plugin's `setupMarkdown` is Markdown its author wrote. Show it only through `SetupSteps`, which reads a small subset (paragraphs, lists, bold, inline code, links) into React elements holding text, never `dangerouslySetInnerHTML` or a Markdown library's HTML, so raw HTML in it stays text (ADR 0058). Link only http and https URLs (`webUrl`), through `ExternalTextLink` (`src/shared/components/external-text-link.tsx`).
- **Plugins aren't sandboxed.** Copy may say "an Installed Plugin can only call what you granted", never "sandboxed" (ADR 0049).
