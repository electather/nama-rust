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

**Server URL**:
The one address an admin sets for people to reach a Server's web app at, such as `https://nama.example.com`. Notifiers' messages link to Requests there; with none set, they have no links.
_Avoid_: base URL, public URL

## People

**User**:
A person's identity on one Server.
_Avoid_: profile, member, local user

**Device**:
A client installation signed in to a Server as a User. Revoking a Device signs it out.
_Avoid_: client, session, token (for the Device itself)

**Sign-in Method**:
A way a User can prove who they are on a Server, such as a password, a passkey or an identity at an outside provider. A User can have several.
_Avoid_: credential, account, login, identity

**Recent Authentication**:
A Device having proved who it is with one of its User's Sign-in Methods in the last 10 minutes, which sensitive changes need.
_Avoid_: sudo mode, step-up, fresh session

**Sign-in Code**:
A short code a new Device shows so a User already signed in elsewhere can sign it in without typing a password on it.
_Avoid_: pairing code, PIN, device code

**Setup Code**:
A one-time code a fresh Server prints to its log, which whoever creates the first Admin must enter.
_Avoid_: claim token, bootstrap code

**Permission**:
One thing a User is allowed to do on a Server, such as Request or manage Libraries. Each User has their own set, and new Users get the Server's default set. A User with the Admin Permission may do everything.
_Avoid_: role, scope, right, group

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

**Package**:
The file an Installed Plugin is loaded from, which gives its Manifest and its code.
_Avoid_: wasm file, module, binary

**Manifest**:
What a Plugin says about itself before it is enabled: its id, name, version, description and logo, the Nama API versions it works with, its settings, the hosts it may reach, and its Extension Points. A Built-in Plugin's comes from its code, an Installed Plugin's from its package. A Server outside those Nama API versions refuses the Plugin, as it refuses an Installed Plugin naming an Extension Point it doesn't know or lacking an export one needs, and a Plugin may take part only at the Extension Points it names.
_Avoid_: descriptor, plugin.json, metadata

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
A Plugin's implementation of a capability the Server needs, such as metadata, checking a User's credentials, or Request fulfillment.
_Avoid_: driver, backend, adapter

**Fulfiller**:
A Provider that hands Approved Requests to an outside service, such as Sonarr or Radarr, and reports how they're progressing.
_Avoid_: downloader, *arr, client

**Notifier**:
A Provider that delivers notifications about Requests to people through an outside service, such as Telegram. The Server decides who is told what; the Notifier only delivers.
_Avoid_: notification agent, channel, alerter

**Connector**:
A Provider that connects a User to an outside service through a link they open, such as a Telegram deep link, and says who they are there. The Kernel keeps that as their Plugin Connection.
_Avoid_: linker, integration

**Plugin Connection**:
A User's identity at an outside service, such as their Telegram chat, that a Plugin acts on for them.
_Avoid_: integration, link, account

## Media

**Library**:
A named collection of one kind of media (Movies or Shows) on a Server, backed by one or more folders.
_Avoid_: collection, section

**Item**:
What a file in a Library holds, going by its name: one Movie, or one or more Episodes of a Show. Each Item is Matched on its own.
_Avoid_: entry, media item, title

**Movie**:
A film, as a metadata source describes it.
_Avoid_: film, title, item

**Show**:
A TV series, as a metadata source describes it, made up of Seasons of Episodes.
_Avoid_: series, TV show, title

**Season**:
One numbered run of a Show's Episodes.
_Avoid_: series

**Episode**:
One installment of a Show, numbered within a Season.
_Avoid_: chapter, part

**Version**:
One of the files a Movie or Episode is available in. A Movie with two Matched files has two Versions.
_Avoid_: copy, edition, file

**Match**:
The link between an Item in a Library and a title in a metadata source. An Item without one is Unmatched.
_Avoid_: identification, lookup

**Pinned Match**:
A Match an admin set or cleared by hand. Automatic Matching follows it instead of searching, so it isn't overwritten.
_Avoid_: locked match, override, manual lock

**ID Hint**:
A title's id at a metadata source, written in a file or folder name, such as `{tmdb-78}` or `[tvdbid-433631]`. An Item with one is Matched by that id without a search.
_Avoid_: id tag, provider id

**Scan**:
One pass over a Library: storing what changed in its folders, then Matching the new Items, then Probing the files without current Tracks.
_Avoid_: pass, sync, refresh

**Probing**:
Reading a Version's file to learn its Tracks and where its keyframes are.
_Avoid_: analysis, inspection

**Watch State**:
A User's progress through a Movie or Episode: whether it's been played, how far in, and when.
_Avoid_: watched status, play state, progress

**Continue Watching**:
A User's Movies and Episodes whose Watch State says they've started and not played, most recent first.
_Avoid_: on deck, in progress, resume list

## Playback

**Track**:
One video, audio or subtitle stream inside a Version, such as its English audio or its French subtitles.
_Avoid_: stream, channel

**Base Layer**:
The picture inside a Dolby Vision Track that a player without Dolby Vision shows instead: HDR10, HLG or SDR. Some Dolby Vision Tracks have none, and only a Dolby Vision player can show them.
_Avoid_: fallback layer, compatibility layer

**Direct Play**:
Playing a Version's file exactly as it is.
_Avoid_: direct stream, original

**Remux**:
Playing a Version with its video copied untouched into a container the client can play, with its audio converted when the client can't play it.
_Avoid_: direct stream, repackage

**Transcode**:
Playing a Version with its video re-encoded, such as when the client can't decode it, a Playback Limit asks for less, or subtitles are burned in.
_Avoid_: convert, encode

**Playback Plan**:
How the Server will play one Version on one Device: which Version, when the client didn't name one; by Direct Play, Remux or Transcode; with which Tracks; and the reasons for each choice. A new one is made whenever what it was based on changes.
_Avoid_: decision, profile, stream info

**Playback Limit**:
The most a User asks playback to send, such as "Original", "1080p" or "720p". A limit below the Version's own makes it Transcode; when the Server can't Transcode it, it plays as it is and the Playback Plan says why. Until the User picks one, a Device outside the local network gets the default an Admin sets for remote playback.
_Avoid_: quality, bitrate, resolution

## Requests

**Request**:
Users asking for a Movie, or one Season of a Show, that isn't on a Server to be added to it. Each Movie or Season has at most one open Request, and everyone who asks for it shares it.
_Avoid_: ticket, order, wish

**Requester**:
A User who asked for what a Request or a Followed Show covers. Each can have several.
_Avoid_: owner, submitter

**Quality**:
One choice an admin offers on a Fulfiller's list for how a Movie or Show is downloaded, such as "HD" or "4K", with a friendly name and one marked as the default. A Request has one. Users allowed to choose pick it, or set their own default for when they don't; everyone else gets the default. Once its hand-off reports what the service used, a Request's Quality is that one, even a service profile the list doesn't offer. A Show has one Quality for all its Seasons: whoever first asks for it sets it, a Request manager may change it, and it is fixed once a Season is handed off.
_Avoid_: quality profile, profile, resolution, tier

**Request manager**:
A User with the Manage Requests Permission, who approves, declines, retries and cancels everyone's Requests.
_Avoid_: approver, admin, moderator

**Followed Show**:
Users asking for a whole Show, approved once, so its Seasons, including ones that come out later, are Requested without asking again. A Season Request appears for a new Season when its Fulfiller fetches it and a Library scan Matches its Episodes. A Season it asked for that was then declined, cancelled or withdrawn, or that was once Available, isn't asked for again.
_Avoid_: monitored show, subscription, series request

**Hand-off**:
Giving an Approved Request to its Fulfiller, which ends with the Request Processing or Failed.
_Avoid_: dispatch, submit, send

**Processing**:
The state of a Request once its Fulfiller has taken it, until a Library scan Matches what it asked for.
_Avoid_: downloading, sent, in progress

**Failed**:
The state of a Request its Fulfiller didn't take, such as when the service is unreachable, kept with the Fulfiller's reason. It stays Failed until a User with Manage Requests retries it, which hands it to the Fulfiller again.
_Avoid_: errored, stuck, timed out

**Partially Available**:
The state of a Request its Fulfiller has, when some, but not all, of what it asked for has been Matched in a Library. A Request not yet handed off keeps its state until all of it is Matched. For a Season, what it asked for is the Episodes aired so far, so a Season still airing can become Available, and Episodes that air later don't reopen it.
_Avoid_: in progress, partial
