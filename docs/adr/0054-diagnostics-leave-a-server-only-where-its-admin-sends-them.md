# Diagnostics leave a Server only where its admin sends them

A self-hosted Server runs on someone else's machine, so nothing it records about itself leaves it unless its admin says so. A Server and its web app record Diagnostics as OpenTelemetry spans, logs and metrics, and export them over OTLP only to an endpoint the admin sets through the standard `OTEL_*` variables. With none set, the Server installs no exporter and pays nothing for it. Sharing Diagnostics with Nama's developers, and separately sharing Usage Analytics, is an opt-in the admin turns on and can turn off. What is shared goes to the Hub, which knows the Server, limits how much it sends and forwards it to whichever backend the developers use. The web app sends its Diagnostics and Usage Analytics through its Server, never straight to a collector or vendor.

Spans, logs and events carry ids, counts, sizes, durations, codecs and outcomes. They never carry titles, file paths, names, IP addresses or search text, so sharing them reveals nothing about what a household watches.

## Considered Options

- **On by default, or a developer's ingest key built into releases**: rejected. It sends a household's data off its Server without the admin choosing it, and an ingest key shipped in an open-source binary lets anyone write to that account.
- **Servers export straight to a vendor (Grafana Cloud, Sentry, PostHog)**: rejected. Every Server would need the vendor's key, and changing vendor would need a release. The Hub holds the keys instead, and the backend can change behind it.
- **The browser exports straight to a collector**: rejected. The collector needs CORS, its address and any key end up in the web app, and ad blockers drop requests to known vendors.
- **Vendor SDKs (Sentry, PostHog) in the Server and web app**: rejected for Diagnostics. OTLP lets an admin use any backend; a vendor SDK ties the code to one.

## Consequences

- A Server that never sets an endpoint and never opts in behaves as before: logs on stdout, and nothing else.
- The web app learns from its Server whether exporting is on, and loads the OpenTelemetry SDK only then, so most people's web app doesn't download it.
- Usage Analytics are a named list in the code, so adding an event is a reviewed change, not something a call site decides.
