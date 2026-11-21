# Nama's non-goals

Some features keep being proposed and Nama will never build them: not in the Kernel, not as a Plugin, and not in a client. Writing them down saves contributors and agents from proposing them again. Each has its reason and what to run beside Nama instead. A new non-goal gets its own ADR.

| Non-goal | Why | Run instead |
|---|---|---|
| Live TV from tuners, program guides (EPG, XMLTV) and DVR | An always-on reliability burden that would compete with playback; it is Jellyfin's least reliable area | Channels or Plex with an HDHomeRun |
| IPTV: M3U and Xtream playlists, provider portals, stream relays | Store takedowns: IPTV Smarters and Perfect Player were pulled from Google Play, and Roku banned an M3U player, though none hosted anything | Jellyfin with Threadfin, or an IPTV player |
| Virtual channels (scheduled streams made from a Library) | Live TV under another name; shuffling a Show covers most of the lean-back need | Tunarr, ErsatzTV |
| `.strm` files, "add by URL" and any other pointer to a remote stream | The Server would fetch any URL it is given (SSRF), playback would depend on someone else's host, and stores pull apps that play arbitrary URLs (Downloader, twice in 2023). It is the same feature as IPTV in a smaller diff | An rclone, NFS or SMB mount, whose files a Library reads like any other |
| Saving media from YouTube or other third-party sites, trailers and channel archives included | Apple 5.2.3 forbids downloading media from third-party sources without authorization | — |
| A client that downloads or runs Plugin code, or browses and installs Plugins | Apple 2.5.2 and 4.7 and Google Play's rules on dynamic code; Kodi has never been on the App Store. Plugins reach clients only as data (ADR 0058) | The web app the Server serves |
| A client that shows how a Request is fetched: indexers, releases, torrents, magnets, Usenet, download-client names, or setting up Sonarr or Radarr | Apple pulled Fetch for adding magnet links, and rejected it again after the feature was gone. Requests themselves stay, shown only in Nama's own states | The Fulfiller's own service |
| DLNA and UPnP media serving | Players see only the files and none of Nama: no Requests, no Watch State per User, no Subtitle Preferences, no access rules. It also means a second, unauthenticated way into a Library (ADR 0089) | Nama's own apps, or casting from them |
| Music, audiobooks, podcasts, books, comics and photos | Each has a dedicated app people already run, and Jellyfin's breadth is part of why it feels cluttered. A Library's kinds are fixed in the Kernel | Navidrome, Audiobookshelf, Immich, Kavita, Komga |

Media is only ever a file the Server reads from a Library's folder. The store reasons cover every client, the web app included, because a takedown of one native client is a takedown of the brand, and the Server can't know which client will show what a Plugin added.

This overrules three lines of `docs/research/jellyfin-feature-research.md`: D3's "if ever, as a Plugin outside the Kernel" for Live TV, "STRM / cloud-drive libraries" and "YouTube / channel archiving".

## Considered Options

- **Ban these from the Kernel only and let Plugins add them**: rejected. No Extension Point can hand the Kernel something to play, so a Plugin would need one built for it, and that puts the reliability burden and the store risk back on Nama's own playback and clients.
- **Copy Silo's list whole** ([Silo's non-goals](https://github.com/Silo-Server/silo-server/blob/main/docs/non-goals.md)): rejected for "never delete media". Nama deletes media on purpose, through Cleanup and ADR 0038.

## Consequences

- Home video is left open, not banned: a kind for personal recordings is plausible once share links exist.
- Adult metadata, paid unlocks and a demo server for store reviewers aren't non-goals. A title source is the Admin's choice on their own Server, Nama charges nothing, and a demo server is work for when the first store client ships.
