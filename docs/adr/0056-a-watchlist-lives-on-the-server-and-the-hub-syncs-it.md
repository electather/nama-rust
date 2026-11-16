# A Watchlist lives on the Server, and the Hub syncs it

A Watchlist is a User's list on their Server, of titles kept by External ID whether or not the Server has them, so a missing title is one step from a Request. The Hub, once it exists, merges a User's Watchlists across their Links as it does Watch State (ADR 0008, 0013), but doesn't own them. This amends ADR 0008, which gave Watchlists only to linked Users.

## Considered Options

- **A Watchlist that belongs to a Hub Account (ADR 0008's plan):** rejected. It waits on the whole Hub, though it's among the most asked-for features in the Jellyfin research (features.jellyfin.org #576, 1,307 votes), and ADR 0002 says a Server is fully usable without the Hub.
