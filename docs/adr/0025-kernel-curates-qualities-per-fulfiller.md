# The Kernel keeps a curated list of Qualities per Fulfiller

Requesters may choose how a title is downloaded, so the Kernel keeps, for each Fulfiller, a list of Qualities: service quality profiles an admin chose to offer, each with a friendly name, and one marked as the default. A narrow `CHOOSE_QUALITY` Permission lets a User pick from that list. Everyone else gets their own default or the Fulfiller's. The server refuses any Quality that isn't on the list. A Request keeps a copy of its Quality, including the service profile id, so a rename or removal doesn't change what it reads or what it hands off. A Show has one Quality for all its Seasons, because Sonarr keeps one profile per series. The Quality is fixed once a Season has been handed off, and the hand-off reports the profile the service actually used.

## Considered Options

- **Seerr's single `REQUEST_ADVANCED` Permission** for quality, root folders, tags and servers together: rejected. Admins can't allow quality without the rest (seerr#2248).
- **Offering every profile on the service, under its raw name**: rejected. Admins can't limit what is offered, and Users see names like "Ultra-HD Remux".
- **Trusting the profile id the client sends**: rejected. A client could ask for any profile on the service.
- **A Quality per Season**: rejected. Sonarr would silently ignore every Season's choice after the first.

## Consequences

- With no list, nothing changes: the Kernel sends no Quality, and the Plugin's own `quality_profile` setting applies.
- If the service no longer has a Request's profile, the hand-off fails with a reason, and a Request manager can retry it with another Quality.
- When Sonarr already had a series, it keeps that series' profile. The Request shows the profile Sonarr used, not the one that was asked for.
