# Settled-Library events report catching up, not passes

A Scan publishes `MatchingFinished` once Matching has decided what it can, and `ProbingFinished` once Probing has caught up, so clients and tests wait on an event, not on knowing the Scan's order. Probing runs outside the scan lock, so passes of one Library overlap, and a pass skips files another prober holds. So `ProbingFinished` means the Library has caught up: a pass waits for the files it skipped, and a pass whose Probing ends after a later pass of the same Library started sends nothing, leaving it to the later one. The API carries no Scan id: a client subscribes, calls `ScanLibrary`, skips to the next `ScanStarted`, and the first of each event after it covers its call.

## Considered Options

- **One event per pass, tagged with a Scan id**: rejected. `ScanLibrary` folds a call into a pass already waiting, so it would have to return an id it doesn't yet own, and "this pass is done" still wouldn't say the Tracks are current while an earlier pass is probing.
- **Report only what this pass probed**: rejected. A pass can finish while another still probes the same Library's files, so the event wouldn't mean what admins and tests wait for.

## Consequences

- A pass that fails after it started, such as on a database error, also silences the Probing it superseded, so a waiter hears nothing until the next Scan. Failures are logged.
- `ScanFinished` keeps its meaning and place: the files are stored, and Matching follows.
