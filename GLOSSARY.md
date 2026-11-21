# Nama

A self-hosted media server and request manager, with an optional central Hub for identity and Server discovery.

## Deployment

**Server**:
The self-hosted program that owns Libraries, streams media to clients, and handles Requests.
_Avoid_: backend, instance, media server, command server

**Hub**:
The central service where Accounts live and where clients find the Servers they can reach. It also keeps a copy of each Account's Watch State, and takes in the Diagnostics and Usage Analytics that Servers share with Nama's developers.
_Avoid_: command server, cloud, plex.tv

**Relay**:
An optional service that carries media traffic between a client and a Server when they can't connect directly.
_Avoid_: proxy, tunnel

**Server Address**:
One way a client might reach a Server, such as a LAN address, a public URL or a Relay. The Hub gives clients a list of them to try.
_Avoid_: endpoint, connection, URI

**Server URL**:
The one address people reach a Server's web app at, such as `https://nama.example.com`: the one an admin sets, else a Linked Server's public name from the Hub. Notifiers' messages and Invites link there; with none, they have no links.
_Avoid_: base URL, public URL

**Diagnostics**:
The logs, errors and timings a Server and its web app record about how they ran. They stay with the Server, go only where its admin sends them, and reach Nama's developers only if the admin shares them.
_Avoid_: telemetry, crash reports, monitoring

**Usage Analytics**:
Named events recording that a feature was used, such as a Request made or playback started, with no titles or names in them. A Server sends them to Nama's developers only if its admin shares them, separately from Diagnostics.
_Avoid_: tracking, telemetry, metrics

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

**Download subtitles**:
The Permission to search a subtitle source for the Version a User is watching and download one of its subtitles.
_Avoid_: subtitle search permission

**Invite**:
A single-use link that lets one person create a User on a Server, with the Permissions chosen when it was made.
_Avoid_: invitation code, share, referral

**Account**:
A person's identity on the Hub, which can be linked to Users on many Servers.
_Avoid_: Hub user, login, profile

**Link**:
The connection between a User and an Account.
_Avoid_: pairing, binding

**Watchlist**:
A User's list of titles to watch later, whether or not their Server has them.
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

**Plugin contract**:
What an Installed Plugin is built against, with a version of its own that a Manifest's requirement is matched against.
_Avoid_: plugin API, SDK, Nama API

**Manifest**:
What a Plugin says about itself before it is enabled: its id, name, version, description and logo, setup steps, a homepage and a source link, a licence, the Plugin contract versions it works with, its settings, the hosts it may reach, the Plugin Permissions it requests, its Extension Points, and the ids its Fulfiller needs. A Built-in Plugin's comes from its code, an Installed Plugin's from its package. An Installed Plugin's names each Extension Point by its service's full name with the versions of it the Plugin works with. A Server outside those Plugin contract versions refuses the Plugin, as it refuses an Installed Plugin naming none or naming an Extension Point it doesn't serve, at a version it doesn't serve, or lacking an export one needs, and a Plugin may take part only at the Extension Points it names.
_Avoid_: descriptor, plugin.json, metadata

**Extension Point**:
A named place where the Kernel lets Plugins take part, declared as Observe or Provide. One an Installed Plugin can serve is a service of the Plugin contract, such as `nama.plugin.v1.NotifierService`, with a version of its own (ADR 0043).
_Avoid_: hook, seam, slot, UI Extension Point

**Observer**:
A Plugin's handler that is told something happened and can't change it.
_Avoid_: listener, subscriber

**Provider**:
A Plugin's implementation of a capability the Server needs, such as finding titles, checking a User's credentials, or Request fulfillment.
_Avoid_: driver, backend, adapter

**Title source**:
A Provider that finds and fetches Movies and Shows by a Namespace of External IDs, such as TMDB or TVDB. It keeps its titles under one Namespace and may look them up by others. Each Library asks its own title sources in its own order (ADR 0044).
_Avoid_: metadata provider, metadata agent, scraper

**Search source**:
A Provider that answers what a User types into search with titles by External ID, in its own order, such as by meaning rather than by words. The Server decides whether each is on it, Requested or not on it; it doesn't fetch titles, as a title source does.
_Avoid_: search provider, search engine, search plugin

**Recogniser**:
A Provider that proposes what a file is, such as an Episode numbered from the start of its Show, or which title an Item names. The Kernel decides: it alone writes Matches and marks an Item Unmatched, and its own Matching is what it falls back to. Each Library asks its own Recognisers in its own order (ADR 0050).
_Avoid_: matcher (the code's `matching::Matcher` fixes Matches by hand), parser, identifier

**Blob store**:
Where the Kernel's own stored files live, such as the originals of its images, Custom Images, Plugin logos and downloaded subtitles: local disk under the data directory by default, or a Provider such as an S3 Plugin, which the Admin picks. It never holds media files, and resized copies stay a local cache (ADR 0052, 0067).
_Avoid_: storage backend, bucket, object store, file host

**Subtitle source**:
A Provider that searches an outside service, such as OpenSubtitles.com, for a Version's subtitles and downloads one a User picks (ADR 0067).
_Avoid_: subtitle provider, subtitle agent, subtitle downloader

**Segment source**:
A Provider that says where a Version's Segments are, such as from an outside service like TheIntroDB. Each Library asks its own Segment sources in its own order, after the Kernel has looked for Segments in the Version's Chapter names.
_Avoid_: segment provider, intro detector, skipper

**Fulfiller**:
A Provider that hands Approved Requests to an outside service, such as Sonarr or Radarr, reports how they're progressing, and has the service stop monitoring what's been deleted, so it doesn't download it again.
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

**Personal setting**:
A setting a Plugin's Manifest declares for each User to set for themself, such as their ntfy topic or Discord webhook. The Admin sets the Plugin's own settings; only the User sees their Personal settings, and a Plugin gets them only in a call for that User (ADR 0046).
_Avoid_: user setting (confused with Settings preferences)

**Automation lane**:
Where a Plugin acts on the Server on its own account rather than answering the Kernel: the Kernel sends it Event batches and runs its Jobs, and it acts through Kernel functions (ADR 0048).
_Avoid_: automation API, scripting

**Kernel function**:
One of the few things any Plugin may ask the Kernel for during any of its calls, each gated by a Plugin Permission the Admin granted, such as reading Watch State or deleting a title's media (ADR 0071).
_Avoid_: host function, automation API

**Job**:
A named piece of work a Plugin's Manifest declares, which the Kernel runs on the Job's schedule, a cron expression in the Server's time zone, or only when an Admin asks if it has none. The Kernel declares none of its own: the Server's own upkeep is a Built-in Plugin's Jobs. An Admin sees every Job, who declared it and its last and next Job Run, and can start one now, turn it off or give it a schedule of their own in place of its default.
_Avoid_: task, scheduled task, cron, Tick, maintenance

**Job Run**:
One time the Kernel runs a Job: when it started and ended, what started it, and how it ended: succeeded, failed, timed out, or interrupted by the Server stopping or the Plugin being disabled. Only one runs at a time for each Job; a firing due while one runs is dropped.
_Avoid_: execution, invocation, Tick

**Plugin Permission**:
One thing a Plugin may ask the Kernel for, such as reading Watch State or deleting media. Its Manifest requests some, and an Admin grants some or all of them when enabling it; the grants survive restarts. Not a User's Permission: it says what the Plugin itself may do, whoever it acts for (ADR 0049).
_Avoid_: scope, capability, Permission (alone, which is a User's)

**Plugin Store**:
The key and value entries the Kernel keeps for one Plugin, each the Plugin's alone or scoped to one User, read and written through Kernel functions.
_Avoid_: plugin database, KV, cache, plugin state

**Plugin Log**:
The lines a Plugin writes about its own work, each naming the Plugin. Part of a Server's Diagnostics, so it stays where the admin sends it, but never shared with the Hub, even when the admin shares Diagnostics.
_Avoid_: plugin output, plugin console

**Cleanup**:
An Automation that deletes a Requested title once each of its Requesters has watched it and a set number of days have passed, or hasn't watched it within a longer set number of days. A title nobody Requested is kept.

**Upkeep**:
The Built-in Plugin whose Jobs delete what the Server keeps that nobody will read again: Home rows of Users who stopped opening home, expired sign-ins and Invites, and what a removed Plugin left behind. Each threshold is one of its settings.
_Avoid_: maintenance, housekeeping, garbage collection

## Media

**Library**:
A named collection of one kind of media (Movies or Shows) on a Server, backed by one or more folders, whose titles come from its title sources, asked in its own order. Its name is unique on its Server, ignoring case.
_Avoid_: collection, section

**Item**:
What a file in a Library holds, going by its name: one Movie, or one or more Episodes of a Show. Each Item is Matched on its own.
_Avoid_: entry, media item, title

**Movie**:
A film, as a title source describes it.
_Avoid_: film, title, item

**Show**:
A TV series, as a title source describes it, made up of Seasons of Episodes.
_Avoid_: series, TV show, title

**Season**:
One numbered run of a Show's Episodes.
_Avoid_: series

**Episode**:
One installment of a Show, numbered within a Season.
_Avoid_: part

**Version**:
One of the files a Movie or Episode is available in. A Movie with two Matched files has two Versions. An Unmatched Item's file is a Version too, which plays it (ADR 0069).
_Avoid_: copy, edition, file

**Taken along**:
An Episode a delete removes without naming it, because a file of what was named holds it too.
_Avoid_: collateral, doomed

**Content Rating**:
The age guidance a country's ratings board gives a Movie or Show, such as PG-13 or TV-MA.
_Avoid_: certification, age rating, parental rating, maturity rating

**Score**:
How well a Movie's or Show's audience liked it, from 0 to 10, as the title source it is Matched to averages it. A title nobody has voted on has none.
_Avoid_: rating (that's Content Rating), stars, user score

**Person**:
Someone who worked on Movies or Shows, known by an External ID, with at most one headshot, shared by all their Roles. A Person has no page.
_Avoid_: actor, talent, cast member

**Role**:
A Person's part in one Movie or Show: a character they play (Cast) or a job they did (Crew), in the order the title source bills them. A Show's Roles span its whole run.
_Avoid_: credit (Credits is a Segment), billing

**Cast**:
A title's Roles that are characters.
_Avoid_: actors, starring

**Crew**:
A title's Roles that are jobs, such as Director or Creator.
_Avoid_: staff, credits

**Trailer**:
A video advertising a Movie or Show, kept as a link to where it is hosted. Nama never plays it.
_Avoid_: clip, teaser, preview

**Similar titles**:
The Movies or Shows a title source says are like a title. A User sees only those in their own Libraries.
_Avoid_: recommendations, related, more like this

**Poster**:
The tall image that stands for a Movie, Show or Season.
_Avoid_: cover, thumbnail, primary image

**Backdrop**:
The wide image shown behind a Movie's or Show's page, and the one a Library is shown by. A Library's is that of the title most recently added to it.
_Avoid_: fanart, art, background, banner

**Still**:
The wide image that stands for an Episode: a frame from it.
_Avoid_: thumbnail, thumb, screenshot

**Title Logo**:
The image of a Movie's or Show's title, lettered as its artwork letters it, shown in place of the title's text. It comes from the artwork lists of its Library's title sources.
_Avoid_: logo, title treatment, clearlogo, wordmark

**Artwork list**:
What a title source has for one title, of every kind, from which Artwork picks a Poster, Backdrop and Title Logo. A Season has its own, of Posters only, which an admin picks from in "Edit images".
_Avoid_: image list, artwork set, gallery

**Custom Image**:
An image an admin uploaded, or picked from a title source's artwork list, to show in place of the one the Server would otherwise show, such as a Movie's Poster or a Plugin's logo. Everyone sees it until it is removed, which brings the other back. A title's stays with the title, whatever happens to its files.
_Avoid_: override, variant, replacement

**Match**:
The link between an Item in a Library and a title in a title source. An Item without one is Unmatched.
_Avoid_: identification, lookup

**Pinned Match**:
A Match an admin set or cleared by hand. Automatic Matching follows it instead of searching, so it isn't overwritten.
_Avoid_: locked match, override, manual lock

**External ID**:
A title's id at a title source: a Namespace and a value that Namespace's owner defines, written `tmdb-603`. A title may have several, at most one per Namespace, and any one of them identifies it.
_Avoid_: title id (for an id the Server makes up), TMDB id (as the general word)

**Namespace**:
Who owns an External ID's values, such as `tmdb`, `imdb` or `tvdb`: one or more lowercase ASCII letters and digits. A title source keeps its titles under one Namespace and may look titles up by others.
_Avoid_: source id, id type

**ID Hint**:
An External ID written in a file or folder name, such as `{tmdb-78}` or `[tvdbid-433631]`. An Item with one is Matched by that id without a search, by the title source that keeps or looks up its Namespace; while no enabled source does, the Item waits undecided.
_Avoid_: id tag, provider id

**Scan**:
One pass over a Library: storing what changed in its folders, then Matching the new Items, then Probing the files without current Tracks.
_Avoid_: pass, sync, refresh

**Probing**:
Reading a Version's file to learn its Tracks and, when the file lists them, its Chapters and where its keyframes are.
_Avoid_: analysis, inspection

**Watch State**:
A User's progress through a Movie or Episode: whether it's been played, how far in, and when.
_Avoid_: watched status, play state, progress

**Continue Watching**:
A User's Movies and Episodes whose Watch State says they've started and not played, and the next Episode of each Show they're partway through, most recent first, leaving out what they've hidden from it. Hiding a title lasts until the User's Watch State of it, or for a Show of any of its Episodes, next changes, as when they play it or mark it played or unplayed, and never changes its Watch State.
_Avoid_: on deck, in progress, resume list, Next Up

**Home row**:
A headed row of titles on a User's home page. The Kernel builds some, such as Continue Watching; a Plugin may provide others, shared by every User or each User's own, which can hold titles not on the Server for the User to Request.
_Avoid_: shelf, section, rail, carousel

**Featured**:
The few Movies and Shows at the top of a User's home page, which the Kernel picks by weight from its own suggestions and those Plugins give. It isn't a Home row: it has no heading, and only titles on the Server can be in it.
_Avoid_: hero, spotlight, banner, billboard

**Home layout**:
The order of a User's Home rows, and which of them and whether Featured are shown. Featured is always at the top. An Admin sets the default Home layout; a User follows it until they change their own, and a row they haven't placed goes to the end of theirs.
_Avoid_: arrangement, customization, home settings, home preferences

## Playback

**Track**:
One video, audio or subtitle stream inside a Version, such as its English audio or its French subtitles.
_Avoid_: stream, channel

**Audio Description**:
An audio Track that narrates what is on screen for people who can't see it, as its file marks it.
_Avoid_: AD, descriptive audio, DVS

**Chapter**:
A named part of a Version, as its file lists it.
_Avoid_: section, part

**Segment**:
A stretch of a Version that the player acts on, of one kind: Intro, Recap or Credits.
_Avoid_: marker, skip marker, media segment

**Preview frame**:
A small picture of a Version at one time, shown while scrubbing.
_Avoid_: trickplay, thumbnail, BIF

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
Playing a Version with its video re-encoded, such as when the client can't decode it, a Playback Limit asks for less, or a subtitle needs Burn-in.
_Avoid_: convert, encode

**Burn-in**:
The Server drawing a subtitle Track into the picture as it Transcodes, for a Device that can't show that Track itself.
_Avoid_: hardsub, hardcoded subtitles, overlay

**Playback Plan**:
How the Server will play one Version on one Device: which Version, when the client didn't name one; by Direct Play, Remux or Transcode; with which Tracks; and the Playback Reasons for it. A new one is made whenever what it was based on changes.
_Avoid_: decision, profile, stream info

**Playback Reason**:
One fact that shaped a Playback Plan, such as a codec the Device can't decode or a Playback Limit the picture is larger than. It either decided the plan, by standing in the way of a method the plan would rather have used, changing what plays or dropping one of the User's choices, or is context.
_Avoid_: explanation, note

**Playback Limit**:
The most a User asks playback to send, such as "Original", "1080p" or "720p". A limit below the Version's own makes it Transcode; when the Server can't Transcode it, it plays as it is and the Playback Plan says why. Until the User picks one, a Device outside the local network gets the default an Admin sets for remote playback.
_Avoid_: quality, bitrate, resolution

**Subtitle Preference**:
What a User wants subtitles to start as when they play something: none, or a language, either full or forced-only, and SDH or not. A User sets one in Settings for everything, which can ask for full subtitles only over audio in another language; picking a subtitle, or turning subtitles off, while playing a Show or Movie keeps one for that Show or Movie, which wins.
_Avoid_: default subtitle, remembered subtitle, sticky subtitle, subtitle choice (a pick in one playback)

**Audio Preference**:
What a User wants the audio to start as when they play something: a language, and the title of the Track they picked. A User sets a language in Settings for everything; picking an audio Track while playing a Show or Movie keeps one for that Show or Movie, which wins.
_Avoid_: default audio, remembered audio, audio choice (a pick in one playback)

**Subtitle Appearance**:
How a User's text subtitles look on every Device: one of four sizes, Small, Default, Large or Largest, and an outline or a box behind them. A styled ASS Track the Device draws, and a burned-in one, keep their own look (ADR 0070).
_Avoid_: subtitle style, caption style, subtitle settings

**Subtitle Offset**:
How far a User shifts one subtitle Track's timing to match the audio, kept for that Track of that Version. It doesn't apply to Burn-in (ADR 0070).
_Avoid_: subtitle delay, subtitle sync, subtitle timing

**GPU**:
A graphics processor in the Server's machine that a Transcode can run on, either dedicated or integrated into the CPU. An Admin picks one, or leaves the Server to prefer a dedicated one. The Server names each it finds, such as "AMD Radeon RX 9070 XT".
_Avoid_: device (that's a client), card, adapter, render node

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
Users asking for a whole Show, approved once, so every numbered Season, and new ones as they come out, is Requested without asking again. Specials, Season 0, are left out: a User who wants them asks for them on their own. A Season Request appears for a new Season when its Fulfiller fetches it and a Library scan Matches its Episodes. A Season it asked for that was then declined, cancelled or withdrawn, or that was once Available, isn't asked for again.
_Avoid_: monitored show, subscription, series request

**Hand-off**:
Giving an Approved Request to its Fulfiller, which ends with the Request Processing or Failed.
_Avoid_: dispatch, submit, send

**Processing**:
The state of a Request once its Fulfiller has taken it, until a Library scan Matches what it asked for. A title's pill says "Downloading" while its Fulfiller reports a download, since that's what's being waited on; the state is still Processing.
_Avoid_: downloading, sent, in progress

**Failed**:
The state of a Request its Fulfiller didn't take, such as when the service is unreachable, kept with the Fulfiller's reason. It stays Failed until a User with Manage Requests retries it, which hands it to the Fulfiller again.
_Avoid_: errored, stuck, timed out

**Partially Available**:
The state of a Request its Fulfiller has, when some, but not all, of what it asked for has been Matched in a Library. A Request not yet handed off keeps its state until all of it is Matched. For a Season, what it asked for is the Episodes aired so far, so a Season still airing can become Available, and Episodes that air later don't reopen it.
_Avoid_: in progress, partial
