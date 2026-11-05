# Nama

A self-hosted media server and request manager, with an optional central Hub for identity and Server discovery.

## Deployment

**Server**:
The self-hosted program that owns Libraries, streams media to clients, and handles Requests.
_Avoid_: backend, instance, media server, command server

**Hub**:
The central service where Accounts live and where clients find the Servers they can reach. It also keeps a copy of each Account's Watch State.
_Avoid_: command server, cloud, plex.tv

**Relay**:
An optional service that carries media traffic between a client and a Server when they can't connect directly.
_Avoid_: proxy, tunnel

**Server Address**:
One way a client might reach a Server, such as a LAN address, a public URL or a Relay. The Hub gives clients a list of them to try.
_Avoid_: endpoint, connection, URI

## People

**User**:
A person's identity on one Server.
_Avoid_: profile, member, local user

**Device**:
A client installation signed in to a Server as a User, with its own session that can be revoked.
_Avoid_: client, session, token

**Role**:
What a User is allowed to do on a Server, either Admin or Member.
_Avoid_: group, permission level

**Account**:
A person's identity on the Hub, which can be linked to Users on many Servers.
_Avoid_: Hub user, login, profile

**Link**:
The connection between a User and an Account.
_Avoid_: pairing, binding

**Watchlist**:
An Account's list of titles to watch later, whether or not any Server has them.
_Avoid_: queue, favorites, wishlist

## Extensibility

**Kernel**:
The fixed core of a Server that Plugins can observe and influence but never replace.
_Avoid_: core, engine, host

**Plugin**:
A unit of trusted functionality that plugs into a Server through Extension Points. Everything outside the Kernel is a Plugin.
_Avoid_: addon, extension, module, agent

**Built-in Plugin**:
A Plugin that ships compiled into the Server.
_Avoid_: core plugin, native plugin

**Installed Plugin**:
A Plugin loaded into a running Server from a package, whether bundled with a release or added by an admin.
_Avoid_: third-party plugin, external plugin, WASM plugin

**Extension Point**:
A named place where the Kernel lets Plugins take part, declared as one of Observe, Intercept or Provide.
_Avoid_: hook, seam, slot

**Observer**:
A Plugin's handler that is told something happened and can't change it.
_Avoid_: listener, subscriber

**Interceptor**:
A Plugin's handler that can let an action continue, modify it, or veto it before it takes effect.
_Avoid_: middleware, filter

**Provider**:
A Plugin's implementation of a capability the Server needs, such as metadata or Request fulfillment.
_Avoid_: driver, backend, adapter

## Media

**Library**:
A named collection of one kind of media (Movies or Shows) on a Server, backed by one or more folders.
_Avoid_: collection, section

**Match**:
The link between a file in a Library and a title in a metadata source. A file without one is Unmatched.
_Avoid_: identification, lookup

**Watch State**:
A User's progress through a Movie or Episode: whether it's been played, how far in, and when.
_Avoid_: watched status, play state, progress

## Requests

**Request**:
A User asking for a Movie, or a set of Seasons of a Show, that isn't on a Server to be added to it.
_Avoid_: ticket, order, wish

**Partially Available**:
The state of a Request when some, but not all, of what it asked for has been Matched in a Library.
_Avoid_: in progress, partial
