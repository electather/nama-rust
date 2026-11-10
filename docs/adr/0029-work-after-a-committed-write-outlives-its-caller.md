# Work that must follow a committed write outlives its caller

hyper drops a handler's future when its client disconnects, resets the HTTP/2 stream, or sent a `connect-timeout-ms` that ran out, but `Database::write` commits whether or not its caller is still waiting. So whatever must follow a committed write, such as publishing its events or handing an Approved Request to its Fulfiller, runs in a task of its own that the handler awaits but can't cancel. Without it, an auto-approved Request was stored Approved with no event and no Hand-off.

## Considered Options

- **Let the handler's cancellation cancel the work**: rejected. The write may already be committed, leaving the database and what clients and Fulfillers were told out of step until a restart.
- **Recover on the next read or a periodic sweep**: rejected. Events can't be recovered after the fact, and a sweep delays every Hand-off it catches.

## Consequences

- A change can't be cancelled once its write is sent; a client that gave up still causes it.
- Every RPC whose write is followed by more work needs this, not only Requests: a Pinned Match publishes after its write the same way.
