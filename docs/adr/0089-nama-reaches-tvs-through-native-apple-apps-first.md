# Nama reaches TVs through native Apple apps first

Households judge a Server by its TV app, and Nama was web-only (#927, `docs/research/jellyfin-feature-research.md` §7 D1). The installable web app with lock-screen controls shipped first (#925). Next comes one native app for iOS and tvOS, then Android TV, then AirPlay and Google Cast from the web app. A native app shows Requests on the TV, which no Jellyfin client does without bolting on Seerr. It plays almost anything directly, and it can keep downloads. AirPlay and Cast reach TVs only from a phone, and each needs signed media URLs or a hosted receiver first.

## Considered Options

- **AirPlay and Cast from the web app first**, the research's pick: rejected as the first step. They put the web player on the TV but never Requests, and Cast's receiver runs into mixed content against a plain-HTTP LAN Server (ADR 0010). They still come, after the native apps.
- **A Jellyfin-compatible API sidecar**: not planned. Infuse, Swiftfin and Findroid would connect on day one, but they hide Requests, and Silo's compatibility layer is 218 files tracking a moving target. Nama won't build one. Anyone who does runs it as a separate process outside the Kernel API (ADR 0007).
- **Tizen and webOS store apps**: not planned. Each is a large build plus store upkeep (Samsung took Jellyfin from 2023-09 to 2026-03). They can be reopened when someone takes on the store process.
- **DLNA**: a non-goal (ADR 0078).

## Consequences

- Casting and offline downloads leave M8's "left out on purpose" list. Downloads come to native apps only (#1422), never to the browser, whose storage quotas and eviction make offline video unreliable.
- Short-lived signed media URLs (research X10) wait until AirPlay, Cast or an external player is scheduled. A native app sends its Device's token as a bearer token (ADR 0014), which a browser `<video>` can't.
- How the Android TV app is built is decided in its own issue.
- PRODUCT.md's Users ("a laptop, tablet or phone") widen to TVs when the tvOS app ships, not before.
