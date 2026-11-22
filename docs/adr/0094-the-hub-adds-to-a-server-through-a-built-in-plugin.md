# The Hub adds to a Server through a Built-in Plugin, and answers only with External IDs

Everything a Server does today stays on the Server (ADR 0002). The Hub only adds features, and each one reaches a Server through one Built-in Plugin that implements the existing Extension Points (ADR 0006): Featured suggestions, Home rows and a Similar source, built from the Account's history and the pooled counts (ADR 0072, 0084, 0093). The same Plugin registers the Server, Links Users, syncs Watch State, Watchlists and Subtitle Preferences, sends Viewing Events, and is the Sign-in Method provider for Accounts (ADR 0014), as local-auth is for passwords. The Kernel learns nothing about the Hub and gains only general Kernel functions the Plugin lacks. A Server that isn't Linked doesn't enable the Plugin.

The Hub never holds or serves TMDB or TVDB content. Its answers are External IDs with weights. The Server's own title source Plugins, with the Admin's own keys, turn them into titles, artwork and charts, and show the attribution those services require. TMDB's terms license only the app holding the key and forbid sublicensing access and "derivatives" of its API. TVDB's forbid distributing its data. A Hub serving their data to every Server would break both.

Once the Hub has enough Viewing Events, it can serve charts of its own, such as what's trending among Nama's Accounts, as Home rows of External IDs, under the same rule.

The Hub's "routing" means Server discovery: it hands clients a list of Server Addresses and checks which can be reached (ADR 0057). It never proxies API calls or media. Recognising files, Matching, images and Transcoding stay on the Server, because they need the files. The project runs one Hub. Its API is protobuf over ConnectRPC in `proto/nama/hub/v1/` (ADR 0007), and Servers call it with the identity they got at Linking (ADR 0011).

## Considered Options

- **A shared metadata cache, so Servers need no TMDB key** (ADR 0091's hint): rejected under TMDB's and TVDB's current terms. It comes back with its own ADR only if TMDB agrees to it in writing.
- **Kernel code that calls the Hub directly**: rejected. Recommendations stay swappable without changing the Kernel only if they come through Plugins (ADR 0072).
- **Moving Matching or metadata off the Server so Servers stay slim**: rejected. A Server would be lessened without the Hub, which is what makes people leave Plex.
- **The Hub proxying client API calls to Servers**: rejected. It would put the Hub in every request's path, and its bill and its outages would reach every Server.
- **Admin-run Hubs as a supported setup**: rejected for now. The AGPL allows it, but only a development override of the Hub's URL is built.

## Consequences

- Every Server still needs its own TMDB key, and Discover's TMDB charts stay in that Plugin.
- Every feature the Hub serves costs the project compute, so each one goes through the Plugin with a stored answer and a time to live, never a call made while a page loads.
