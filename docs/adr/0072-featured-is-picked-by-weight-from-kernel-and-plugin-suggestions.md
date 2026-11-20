# Featured is picked by weight from the Kernel's and Plugins' suggestions

The top of a User's home page, Featured, shows the 5 heaviest of the titles suggested for that User. The Kernel suggests its own, and any Plugin may suggest more. We want recommendations to improve without changing the Kernel, so the Kernel holds only a merge rule and Plugins hold the ranking logic. The Kernel suggests the User's Continue Watching entries at 0.8 and the titles added in the last 14 days at 0.5, each fading with age, so Featured isn't empty on a Server with no Plugins. A Plugin suggests through a `user_featured` export on `HomeRowService`. The Kernel calls it per User ahead of time with a time to live, as it calls `user_rows` (ADR 0058), and never while home loads. Each suggestion is a title by External ID with a weight from 0.0 to 1.0, which the Kernel clamps. A title suggested twice keeps its highest weight, and ties go to the title added most recently. The Kernel drops any suggestion it can't find in a Library, because Featured's main button is Play. Featured gives no reason a title is shown: the web app words "Continue watching · 38m left" from the Watch State and shows no kicker otherwise.

## Considered Options

- **Each suggestion carries a reason (Continue Watching, New Episodes, Added this week)**: rejected. A reason enum grows with every Plugin's idea of why, and the web app can word the one reason that matters from the Watch State.
- **An Admin factor per Plugin on its weights**: deferred. A Plugin can claim 1.0 for every title and fill Featured. Until an Admin needs to rein one in, enabling the Plugin is the Admin's trust in it.
- **The Kernel takes turns between sources**: rejected. Turns would keep a weak suggestion over a strong one just because of where it came from.
- **A separate `FeaturedService` Extension Point**: rejected. Asking per User ahead of time and storing with a time to live is already `HomeRowService`'s machinery.
- **Asking Plugins while `ListFeatured` runs**: rejected for the reasons ADR 0058 gives for Home rows.
- **Top Picks built into the Kernel**: rejected. "For you" and "Family night" became a Plugin's Home rows, so their ranking and the Family Content Ratings can change without a Kernel release.

## Consequences

- Featured waits on per-User Plugin calls (#1009) for Plugin suggestions. Until then, it shows the Kernel's suggestions only.
- Recommendation Plugins need Kernel functions for genres, Content Ratings, one User's Watch State and their Watchlist.
