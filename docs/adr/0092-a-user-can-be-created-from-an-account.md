# A User can be created from an Account and signs in through the Hub

An Admin can create a User from a Hub Account instead of giving them a local password. That User is Linked from the start, and their Account is their only Sign-in Method. A local User who Links later ends up the same. There is still one User model (ADR 0001, 0014), so Permissions, Watch State and Plugins never ask how a User signs in. This amends ADR 0008, where every User started local.

The Server still writes Watch State locally first and sends each change to the Hub, best effort (ADR 0008, 0013). Each Server owns the plays made on it, and the Hub owns the merged state across Servers. When a Linked User reaches a new Server, the Hub hands it their Watch State, Watchlist (ADR 0056) and Subtitle Preferences (ADR 0035), all keyed by External ID or described rather than tied to a Track, so they fit any Library. Home layouts, Permissions, Library access and Plugin settings belong to each Server and never travel.

Accounts sign in with a password, a passkey or a magic email link.

## Considered Options

- **Two kinds of User, "local" and "hub", stored apart**: rejected. Every feature would branch on the kind.
- **Servers read a Linked User's Watch State live from the Hub**: rejected. Resume and Continue Watching would stop whenever the Hub is down, breaking ADR 0002.

## Consequences

- While the Hub is down, a User who signs in with their Account keeps any session they already have (ADR 0011's cached keys), but can't start a new one.
- A Server always keeps at least one Admin who can sign in locally, so a Hub outage never locks an Admin out of their own Server.
- A title the new Server lacks keeps its state on the Hub, and that state shows up once the title is added.
