# The Kernel keeps Plugin Connections and decides who is notified

Users get personal notifications, for example in their own Telegram chat, but Plugins have no storage (ADR 0014). So the Kernel keeps one opaque value per User and Plugin, a Plugin Connection, and the Plugin connects a User through a link they open. Telegram's bot is reached by polling while the link is pending, because its webhooks and Login Widget need the Server on a public domain, which most home Servers aren't. The Kernel, not the Plugin, decides who hears about what, from Requesters and Permissions, and hands each notification to a Notifier with one recipient: a Plugin Connection or the Plugin's channel.

## Considered Options

- **Users type their Telegram chat id**, as Seerr does: rejected. Members aren't technical.
- **Notifiers as Observers of every event, picking recipients themselves**: rejected. Every Notifier would repeat the same rules and need Permission and Requester data.

## Consequences

- A Notifier that finds a recipient gone (Telegram answers 403) says so, and the Kernel deletes that Plugin Connection.
- Trakt scrobbling (M8) can reuse Plugin Connections.
