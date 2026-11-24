# Plugins log through a Kernel import the Hub never sees

An Installed Plugin could already log by accident: `nama-pdk` re-exports `extism_pdk`, whose `info!` and friends reach Extism's own host functions, which emit a `tracing` event tagged only with the instance's UUID. Those host functions are built into Extism's `extism:host/env` module, so the Server can't add the Plugin's id to them, blank its secret settings, cap them or keep them for the Plugins page. So a Plugin logs through `nama.log`, an import of the Plugin contract (ADR 0041) and one more row of the Kernel's `nama` functions, which the Plugin calls through `nama_pdk::{debug!, info!, warn!, error!}`. A line is a level and plain text, formatted inside the Plugin; it has no key and value fields, since a field the Server can't name at compile time could only arrive as one more string, and every extra place for text is one more place for a secret. `nama-pdk` stops re-exporting Extism's log macros, so calling one fails to build, and `extism::pdk` is off in both of the Server's filters. Adding the import is a minor bump of the Plugin contract.

What a Plugin writes is its Plugin Log. Every line names its Plugin's id where stdout shows it. An Installed Plugin's line carries it as a field of the line itself, under one target, `nama::plugin`. A Built-in Plugin logs with `tracing` from its own code, so the Registry runs each call to it in a span with target `nama::plugin` holding its id; stdout hides only spans under the Server's own target, so it prints this one. The Kernel still can't tell the two kinds apart in what an admin reads. The levels are `debug`, `info`, `warn` and `error`, filtered by `RUST_LOG` like the Server's own; the default is `info`, and an admin sees one Plugin's debugging with `nama::plugin=debug`. A Plugin needs no Plugin Permission to log, since a line reaches nothing outside the Server.

The Server has to run well on a machine with 1 GiB of RAM (ADR 0039), so the Server bounds what a Plugin writes. A line over 2 KiB is cut there and marked as cut. Each Installed Plugin has a bucket of 10 lines a second with a burst of 50; a line over it is dropped and counted, and the Server logs one `warn` a minute naming the Plugin and how many lines it dropped. A line the level filter drops costs nothing from the bucket. A Built-in has no bucket, since it is the Server's own code. Unlike the other `nama` imports, `nama.log` doesn't pause the call's clock, so a Plugin stuck logging in a loop still meets its time limit. Before writing a line, the host blanks the Plugin's secret setting values in it, as it does in a failure's message. That catches only a secret written as stored, so the `nama-pdk` README says never to log a setting's value, a token or a response's body. A call made with no enablement, to read a Manifest or check settings, has no Plugin Log: its lines are dropped.

Plugin Logs are Diagnostics, so they go to stdout and to the admin's own collector like any other line (ADR 0054). They never reach the Hub, even when the admin shares Diagnostics: the Hub takes only log lines marked shared in the code, each reviewed, and a Plugin's text, written by a third party, can't be reviewed.

The Plugins page shows an admin each enabled Plugin's last 100 lines that passed the filter, kept in memory and cleared when the Plugin is disabled or the Server restarts, so at most about 200 KiB a Plugin.

## Considered Options

- **Document Extism's macros as they are**: rejected. Lines stay unattributed, unbounded and unblanked, and the contract would quietly depend on Extism's host interface.
- **Re-export Extism's macros as `nama_pdk::log`**: rejected. A name Nama owns with nothing Nama controls behind it.
- **A span with the Plugin's id around each Installed Plugin call**: rejected. Extism's line is emitted from inside Extism, but the host function is ours now and can put the id on the line directly, which costs nothing and survives any filter.
- **Key and value fields on a line**: rejected. `tracing` needs field names at compile time, so a Plugin's keys would arrive as one string, which can be searched but not queried, and another place for a secret to slip past the blanking.
- **Warnings and errors shared with the Hub**: rejected. A Plugin's warning is as likely to quote a path or a title as its other lines, and ADR 0054's shared lines are lines someone reviewed.
- **No Plugins page view**: rejected. Many admins run Nama on a NAS and never read its stdout.

## Consequences

- A Plugin built against an earlier `nama-pdk` that calls Extism's macros logs nothing once the Server switches `extism::pdk` off. No Plugin in `plugins/` does.
- A per-Plugin level set in the UI is left until a Plugin author asks for one.
