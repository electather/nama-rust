# Plugins declare Jobs and the Kernel runs them on cron

Amends ADR 0048: Jobs replace its Tick.

Some state the Server keeps only grows or goes stale, such as Home rows of Users who stopped visiting (#1005), and an Automation Plugin had one unnamed Tick on an interval that no Admin could see or start. We decided a Plugin's Manifest declares named Jobs, each with an optional 5-field cron schedule read in the Server's time zone, and the Kernel runs them through a `JobService` Extension Point, records each Job Run, and shows them all to the Admin on one Jobs page. The Kernel declares no Jobs of its own, so it stays small: the Server's own upkeep is the Built-in Upkeep Plugin's Jobs, which prune through Kernel functions gated by Plugin Permissions like any other Plugin's. Jellyfin, Silo and nama-stream were read first (`docs/research/jobs-comparison.md`).

## Considered Options

- **Keep Tick and add Jobs beside it**: rejected. Two ways to run work on a schedule, one invisible.
- **The Kernel declares its own maintenance Jobs**: rejected. Each would be Kernel code that a Plugin, reaching state through Kernel functions, can do as well.
- **An interval schedule**: rejected. Maintenance belongs at a quiet hour, which only a time of day says.
- **Storing the Admin's whole schedule once edited**, as Jellyfin does: rejected. A Plugin update's new default would never reach that Job.

## Consequences

- `AutomationService.tick` and `Manifest.tick_every_seconds` go; Cleanup moves to one Job in the same change. `automation.proto` still documents the Kernel functions.
- The Kernel refuses a Manifest whose cron doesn't parse, and a `JobService` Plugin declaring no Jobs. A Job with no schedule runs only when an Admin starts it.
- Only one Job Run at a time per Job: a firing due while one runs is dropped, unrecorded, and "Run now" is refused. Firings missed while the Server was down are not made up. Different Jobs run at once.
- A Job Run's row is written when it starts and ends `succeeded`, `failed`, `timed out` (60 s of the Plugin's own time, Tick's limit) or `interrupted`; a row still running at startup becomes `interrupted`. There is no cancel and no retry: the next firing is the retry. A failed Run counts toward disabling the Plugin as a failed call does. The last 50 Runs of each Job are kept, whatever their outcome, so the history doesn't become #1005's next case.
- Plugin Log lines written during a Run carry its id, so its detail shows them.
- The Admin's changes, Enabled and a cron override, are stored only for Jobs they changed; Reset to default deletes the row. They survive disabling the Plugin, and go with the Job when an update drops it or the Plugin is removed.
- A Built-in Plugin always holds every Plugin Permission it requests, since it is compiled in and grants don't limit what it reaches (ADR 0049): each start saves them as its grants, so a later version's new one works on a Server that already had it, and an Admin who wants it to stop doing something turns that Job off or disables it. An Installed Plugin still waits for the Admin.
- Requests are not pruned: Cleanup keeps a title nobody Requested, so pruning available Requests would keep their titles for good.
