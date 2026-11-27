# A Show on the Watchlist is its Followed Show

Adding a Show to a Watchlist on the Server follows it, for a User who may Request Shows: they join its open Followed Show, or open one, approved once as before. Taking it off withdraws them. Following no longer asks for the Seasons already out: the Followed Show covers them, and the User asks for those from the Show's page, so it Requests only each new Season as it comes out. A Season a follower asks for opens for their Followed Show, in its state, so its hand-off is what asks the Fulfiller to follow the Show. This extends ADR 0056: the Watchlist still lives on the Server, and is now also how a User follows a Show.

## Considered Options

- **Keep the Followed Show apart from the Watchlist, behind its own button:** rejected. The redesigned Show page would carry two controls for one wish: "keep me in touch with this Show".
- **Following asks for every Season out, as it did:** rejected. Bookmarking a long Show would download all of it; the Show's page asks for Seasons one at a time, or for all, on purpose.
- **Each new Season waits for approval:** rejected. An admin approves a Followed Show once, as before.

## Consequences

- Only `AddToWatchlist` on the Server follows. A Show added on the Hub and synced to the Server is on the Watchlist without being followed.
- A hand-off alone asked the Fulfiller to follow, so a Show watchlisted with no Season asked for through it, or whose Seasons were handed off before it was followed, got no new Seasons. `Fulfiller::follow` closes the gap (#2485): the Kernel asks the Shows Fulfiller to follow each time a Followed Show becomes Approved, with or without a hand-off.
