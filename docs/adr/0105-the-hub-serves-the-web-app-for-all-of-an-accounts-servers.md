# The Hub serves the web app for all of an Account's Servers, and the Kernel talks to the Hub

_Supersedes [ADR 0010](0010-hub-does-not-host-media-web-app.md), [ADR 0101](0101-the-hub-web-app-shares-only-nama-ui-and-the-web-app-stays-at-webs-root.md) and the Built-in Plugin half of [ADR 0094](0094-the-hub-adds-to-a-server-through-a-built-in-plugin.md)._

_Amends [ADR 0011](0011-hub-trust-device-flow-and-signed-tokens.md)._

The Hub felt like a second product: its own pages, where a person typed Link codes and followed plain links out to Servers. What the Hub is for is one place for everything an Account can reach. That means the content of every Server the Account is Linked on, whether owned or shared, and the Settings of every Server it owns, the way Plex's one web app does both. So there is one web app and one build. Each Server serves it showing only that Server. The Hub serves the same `dist` and, finding itself at the Hub on startup, adds what only makes sense there:
- Home, Continue Watching, the Watchlist and Search merged across the Account's Servers, with one title page per External ID and a choice of Server to play from;
- each Server's Libraries under that Server's name;
- Account settings and the Hub's sign-in pages;
- a Server picker in Settings that runs the same Settings screens against any Server where the Account's User is an Admin;
- a notice to set up a Server or ask for an Invite when the Account has no Linked User anywhere.

Configuring a Server stays complete in its own web app, so no one needs the Hub to run one.

The page at the Hub calls each Server's API directly, over the Server's own HTTPS name (ADR 0057, 0104). The Hub still never proxies API calls or media. Only the Hub's origin may make those calls, never another Server's, because the Account token is what reaches every Server. If a Server's page held it, that Server's Admin could change the JavaScript it serves and act as the Account on every other Server. So:
- The Account token is an HttpOnly cookie on the Hub's origin.
- The page asks the Hub for a short-lived token for each Server, signed as ADR 0011 describes, and trades it at that Server for a Device with a bearer token. That Server lists the Device, can revoke it, and keeps accepting it while the Hub is down (ADR 0096).
- A Server answers CORS only for the Hub's origin, which it reads from its own config, never from the request, and only for bearer requests, never with its cookie (ADR 0015).
- `<img>`, `<video>` and subtitle tracks can't send a bearer token, so a Server gives bearer callers short-lived signed URLs: an image's for its one path, a Playback Plan's for its one Version and that plan. A plan's HLS segments, subtitles, fonts and Preview frames load under the same signed prefix as its URL, and a player keeps fetching under it for as long as it plays, so that prefix keeps working past its few minutes while the plan runs, and stops when the plan does.
- Each Server reports its version and capabilities. The page hides what a Server lacks, and a Server below a minimum version shows as "Update this Server", linking to its own web app.

Letting the Hub's origin reach admin RPCs trusts the Hub with nothing new: it can already sign anyone in as any User on any Linked Server.

The Hub is also no longer a Plugin. The Kernel itself Links the Server and Users, signs Users in through an Account, syncs Watch State, Watchlists and Subtitle Preferences, and sends Viewing Events, so the Server web app can show Hub Settings as part of the Server rather than behind a generic link point. The Kernel stays off the Hub until the Server is Linked, and an unlinked Server loses nothing (ADR 0002). Linking a Server from the web app becomes a redirect: the Admin signs in on the Hub, approves, and lands back in Settings. The code flow stays for a Server with no browser at hand. The Hub's suggestions stay a Built-in Plugin, Hub Suggestions, built on Kernel functions, because they already fill Featured, Home rows and Similar through those Extension Points and should stay swappable (ADR 0072). The Kernel publishes Server and User Linking and Unlinking on the Events point, so Notifiers can announce them. It adds no Hub Extension Point until a second Plugin needs one (ADR 0024). ADR 0094's other half stands: the Hub answers only with External IDs.

## Considered Options

- **The Hub address redirects to one of the person's Servers, and every Server's web app shows all of the Account's Servers**: rejected. Every Server's origin would hold the Account token, or a token minted from it, and would need CORS from every other Linked Server.
- **The Hub keeps an index of every Server's catalogue**: rejected. It would hold TMDB and TVDB titles and artwork, which ADR 0094 rules out.
- **A Server fetches other Servers' content for the browser**: rejected. That Server would become a proxy, and its bandwidth and outages would reach everyone.
- **A separate Hub web app sharing the Server web app's features through a package**: rejected. Two apps to maintain, where one with features switched on at the Hub does the job.
- **Two builds, leaving Hub-only code out of Servers' bundles**: rejected. It's one more artifact to test, and the Hub-only code holds nothing secret.
- **The Hub web app shows an overview and opens each Server's own web app for Settings**: rejected. "Manage all my Servers in one place" means their Settings too, as Plex shows them. Version gaps are handled by capabilities instead.

## Consequences

- An HTTPS page at the Hub can't call a plain-HTTP LAN Server, so the merged view needs ADR 0057's certificates first. Until a Server has them, or the Admin's own HTTPS proxy, the Hub lists it with a link to open it.
- The Hub's web app is always the latest release while Servers run older ones. Capabilities and the minimum version replace any promise that every Server matches the page.
- `web/hub/` goes away, and its sign-in and Account pages move into the web app. ADR 0101's nama-ui package stays as the home of the design system.
- A Hub deploy takes its web app down briefly with it (ADR 0096). Each Server's own web app keeps working.
- The Hub Plugin's stored data moves into Kernel tables.
