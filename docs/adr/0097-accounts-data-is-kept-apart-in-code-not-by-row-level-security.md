# Accounts' data is kept apart in code, not by row-level security

Every Hub table holding an Account's data has an `account_id`, and every query on it is scoped by the Account the call authenticated as, which the Hub's auth layer resolves before any handler runs. Each RPC that reads an Account's data has an integration test showing that another Account's rows don't appear.

## Considered Options

- **PostgreSQL row-level security**: rejected. The database would enforce the scoping, but every transaction on a pooled connection would first have to `SET LOCAL` the Account, and a missed one or a wrong policy shows up as rows silently missing, not as an error a test catches. It comes back with its own ADR if a leak ever gets past review.

## Consequences

- Pooled counts (ADR 0093) and title vectors (ADR 0095) belong to no Account and have no `account_id`.
