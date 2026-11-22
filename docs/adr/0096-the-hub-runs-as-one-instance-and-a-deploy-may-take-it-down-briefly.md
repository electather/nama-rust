# The Hub runs as one instance, and a deploy may take it down briefly

The project runs one Hub (ADR 0094) as a single process against one PostgreSQL database. A deploy stops it, migrates and starts the new one, so the Hub can't be reached for that time. That is acceptable because a Server never needs the Hub (ADR 0002), Servers verify Hub-signed tokens against the key set they cached (ADR 0011), and everything the Hub serves reaches a Server through the Hub Plugin as a stored answer with a time to live (ADR 0094). With one process, a migration needn't keep the previous release working, periodic work needs no lease, and rate limits can live in memory.

## Considered Options

- **Several replicas behind a load balancer, deployed one at a time**: rejected for now. Every migration would have to work with the previous release as well (expand, then contract), every periodic job would need a lease in PostgreSQL, and rate limits would need a shared store, which ADR 0095 already declined Redis for. Nothing the Hub does yet needs a second process.

## Consequences

- Moving to replicas takes all three changes at once: migrations under an advisory lock that keep the previous release working, leased jobs, and shared rate limits. Each place that assumes one process says so in a `ponytail:` comment.
- An Account sign-in, a Linking or a Server discovery that starts during a deploy fails, as it would in any Hub outage (ADR 0092).
- Heavy batch work, such as importing Wikidata's dump (ADR 0095), runs as a subcommand of the same binary that the host's scheduler starts as a separate process, so it never shares the serving process's memory. Light periodic work runs inside the serving process.
