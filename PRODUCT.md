# Product

## Register

product

## Users

Primarily household members: family and friends of whoever runs the Server. They are not technical. They open the web app on a laptop, tablet or phone, often on the couch, to find something to watch, pick up where they left off, or ask for a Movie or Show the Server doesn't have yet. Their job is "start something good in under a minute" and "ask for that thing and know when it's here".

Secondary: the admin who runs the Server. They manage Libraries, fix Matches, enable Plugins and approve Requests. Admin tools exist in the same app but stay out of members' way.

## Product Purpose

Nama is a self-hosted media server with Requests built in (ADR 0001). The web UI is served by the Server itself (ADR 0004, 0010) and is where people browse Libraries, play media, and make and follow Requests.

Success means:

- It feels better than Plex or Jellyfin: less clutter, faster to find and start something.
- Requests feel native. Asking for a title and watching it move to Available is one continuous flow, not a bolted-on Seerr.
- Admin is calm and clear. Libraries, Matching and Plugins are understandable without reading docs.

## Brand Personality

Warm, quiet, confident. Like a good living room: it gets out of the way and lets posters, stills and artwork carry the mood. Copy is plain and friendly, uses the GLOSSARY.md terms, and never sounds like a dev tool. Confidence comes from restraint, not from loud chrome.

## Anti-references

- **Plex's busy home**: rows of promoted content, upsells for services, sidebars full of things you don't own.
- **Netflix clones**: generic black-and-red, hero carousels, autoplaying trailers.
- **The *arr / admin-panel look**: Bootstrap-ish tables, dense settings forms, a developer aesthetic shown to members.
- **Jellyfin's dated chrome**: heavy cards, inconsistent spacing, settings sprawl.

## Design Principles

1. **Your media, nothing else.** Every surface shows what's on this Server or what you asked for. No promotion, no filler rows.
2. **Content carries the colour.** Artwork is the loudest thing on screen; the interface stays quiet around it.
3. **Requests are part of browsing.** A missing title is one action away from being requested, and its state lives where the title lives.
4. **Admin power, member calm.** Advanced controls exist but are progressively disclosed; a member never trips over them.
5. **Plain words.** Use the glossary's language, explain states in human terms, never expose internals.

## Accessibility & Inclusion

WCAG 2.2 AA. AA contrast everywhere, including text over artwork. Fully keyboard operable with visible focus. Captions and subtitle controls are first-class in the player. `prefers-reduced-motion` is respected for every transition. State is never conveyed by colour alone (Request states, Match status).
