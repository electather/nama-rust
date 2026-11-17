# Declared schema breaks

CI runs `buf breaking` on the client API, `proto/nama/v1/`, against main on every change (ADR 0007). A change that breaks it on purpose declares it here, and CI then prints the breaks and passes (ADR 0032). A change under `proto/nama/plugin/` needs no entry: the Plugin contract carries its own version, which says what a Plugin built before can still use (ADR 0041). How to add one is in [`crates/nama-proto/README.md`](../crates/nama-proto/README.md).

Newest entry first. Each entry is a `## YYYY-MM-DD, #<issue>` heading, dated the day the change merges, followed by:

- each rule and file `buf breaking` reports;
- what changed in the schema;
- what a client built before the change must do.

CI counts those headings, so a break passes only with one more than main has: editing an entry passes nothing, and deleting one is a change of its own.

## 2026-11-17, #880

- None that `buf breaking` reports: in `nama/v1/match.proto`, `ListUnmatchedItemsRequest.library_id` (1) is deleted with its number and name reserved.
- `ListUnmatchedItems` pages through the shared messages of `nama/v1/page.proto` (ADR 0055): its request takes the Library in `filter` (2), an `UnmatchedItemFilter`, the order in `sort` (3), an `UnmatchedItemSort` of path or when found, and `direction` (5), a `SortDirection`, and the page in `page` (4), a `PageRequest`; its response gives the next token, and on the first page the `total_size`, in `page` (3), a `PageResponse`. `unparsed_paths` is filled on the first page only.
- A client built before has its Library ignored, so it is refused with `not_found`; it must regenerate and send `filter` and `page`, and read `unparsed_paths` from the first page. The web app is the only client.
## 2026-11-17, #884

- `FIELD_SAME_TYPE`, `nama/v1/browse.proto`: `ListMoviesResponse.movies` (1) and `ListShowsResponse.shows` (1).
- `ListMovies` and `ListShows` page through the shared messages of `nama/v1/page.proto` (ADR 0055): each request takes a `MovieFilter` or `ShowFilter` in `filter` (2), a `MovieSort` or `ShowSort` in `sort` (3), a `SortDirection` in `direction` (4) and a `PageRequest` in `page` (5), and each response gives a `PageResponse` in `page` (2). A listed title is a `MovieSummary` or `ShowSummary`, with only what a grid shows, each field under the number it has in `Movie` or `Show`; `GetMovie` and `GetShow` give the rest, Versions and Tracks among it. The new `ListGenres` lists a Library's genres to filter by.
- A client built before gets the first 50 titles by title, each with its id, title, poster and date, and for a Movie the caller's Watch State, but no Versions or other details; it must regenerate, page with `page`, and read the rest from `GetMovie` or `GetShow`. The web app is the only client.

## 2026-11-17, #882

- None that `buf breaking` reports: in `nama/v1/browse.proto`, `ListContinueWatchingRequest` gains `page` (1), a `PageRequest`, and `ListContinueWatchingResponse` gains `page` (2), a `PageResponse`.
- `ListContinueWatching` lists a page of Continue Watching (ADR 0055), most recent first, where it listed all of it.
- A client built before gets only the 50 most recent entries and finds no next token; it must regenerate and send `page`. The web app is the only client.

## 2026-11-17, #1007

- `MESSAGE_NO_DELETE`, `nama/v1/plugin.proto`: `RecogniserFailure`.
- Not reported by `buf breaking`: in `nama/v1/plugin.proto`, `Plugin.recogniser_failure` (13) is deleted with its number and name reserved.
- A Plugin's failures are kept per Extension Point: `Plugin.failures` (14) lists an `ExtensionPointFailure` for each point whose last call failed, naming the `point` with the same last error, time and calls in a row `RecogniserFailure` held.
- A client built before finds no Recogniser failure; it must regenerate and read `failures`. The web app is the only client.

## 2026-11-17, #879

- None that `buf breaking` reports: in `nama/v1/request.proto`, `ListRequestsRequest.state` (1), `page_size` (2) and `page_token` (3), and `ListRequestsResponse.next_page_token` (3), are deleted with their numbers and names reserved.
- `ListRequests` pages through the shared messages of the new `nama/v1/page.proto` (ADR 0055): its request takes the state in `filter` (4), a `RequestFilter`, and the page in `page` (5), a `PageRequest`, whose `size` is now an `int32`; its response gives the next token, and on the first page the `total_size`, in `page` (4), a `PageResponse`. `REFUSAL_REASON_PAGE_TOKEN_INVALID`'s `field` is now `page.token`.
- A client built before has its state and page ignored, so it gets the first 50 of every state, and finds no next token; it must regenerate and send `filter` and `page`. The web app is the only client.

## 2026-11-16, #833

- `RPC_NO_DELETE`, `nama/v1/match.proto`: `MatchService.SearchMetadata`.
- `MESSAGE_NO_DELETE`, `nama/v1/match.proto`: `SearchMetadataRequest` and `SearchMetadataResponse`.
- `ENUM_VALUE_SAME_NAME`, `nama/v1/plugin.proto`: `ExtensionPoint` 6.
- `ENUM_VALUE_SAME_NAME`, `nama/v1/refusal.proto`: `RefusalReason` 29.
- The client API says title source, as `GLOSSARY.md` does, in place of metadata provider or source. `MatchService.SearchMetadata` is now `SearchTitles`, taking `SearchTitlesRequest` and answering `SearchTitlesResponse`, with the same fields. `EXTENSION_POINT_METADATA_PROVIDER` is now `EXTENSION_POINT_TITLE_SOURCE` and `REFUSAL_REASON_NO_METADATA_PROVIDER` is now `REFUSAL_REASON_NO_TITLE_SOURCE`, keeping their numbers, 6 and 29.
- A client built before calls a search path the Server no longer serves. Over JSON, it drops `EXTENSION_POINT_TITLE_SOURCE` from a Plugin's extension points, because it doesn't know the new name. It still reads `REFUSAL_REASON_NO_TITLE_SOURCE` by its number, since refusal details are binary. It must regenerate and call `SearchTitles`. The web app is the only client.

## 2026-11-13, #833

- `ONEOF_NO_DELETE`, `nama/v1/match.proto`: `ExternalId.id`.
- Not reported by `buf breaking`: in `nama/v1/request.proto`, `RequestedMovie.tmdb_id` (1), `RequestedSeason.show_tmdb_id` (1), `show_imdb_id` (5) and `show_tvdb_id` (6), `FollowedShow.show_tmdb_id` (2), `CreateRequestRequest.movie_tmdb_id` (1), `CreateSeasonRequestsRequest.show_tmdb_id` (1) and `CreateFollowedShowRequest.show_tmdb_id` (1) are deleted with their numbers and names reserved.
- Not reported by `buf breaking`: in `nama/v1/request.proto`, `MovieHit.tmdb_id`, `FoundMovie.tmdb_id`, `ShowHit.tmdb_id` and `FoundShow.tmdb_id` (each 1), `LookUpMovieRequest.movie_tmdb_id` (1) and `LookUpShowRequest.show_tmdb_id` (1) are deleted with their numbers and names reserved.
- A title's `ExternalId` is now a `namespace` (4) and a `value` (5), such as `tmdb` and `603`, in place of the oneof of `tmdb` (1), `imdb` (2) and `tvdb` (3), whose numbers and names are reserved (ADR 0042). The Server refuses an id with a malformed namespace or an empty value with `REFUSAL_REASON_TITLE_ID_MALFORMED`.
- A Request's title is an `ExternalId`: `RequestedMovie.id` (4) with every id the metadata source gave in `ids` (5), `RequestedSeason.show_id` (7) with `show_ids` (8), and `FollowedShow.show_id` (9). `CreateRequest` takes `movie_id` (3), `CreateSeasonRequests` and `CreateFollowedShow` take `show_id` (4 and 3); an id the Server can't look up is refused with `REFUSAL_REASON_UNKNOWN_TITLE` (ADR 0042). `REFUSAL_REASON_ALREADY_IN_LIBRARY`'s `tmdbId` param is now `titleId`, the id's text form (`tmdb-949`).
- A search hit and a looked-up title carry their `ExternalId` in `id` (12 in each), and `LookUpMovie` and `LookUpShow` take `movie_id` and `show_id` (2) (ADR 0042). A look-up of an id no metadata source has, or can look titles up by, is answered `not_found` with the new `REFUSAL_REASON_TITLE_NOT_FOUND` (71) and its `titleId`, in place of `invalid_argument` with `REFUSAL_REASON_UNKNOWN_TITLE`.
- A client built before reads no id from a title and sends none the Server takes; it must regenerate and read and send an id as `namespace` and `value`. The web app is the only client.
- A client built before reads no title id from a Request or Followed Show, and every create it sends is refused for a missing id; it must regenerate and send and read `ExternalId`s. The web app is the only client.
- A client built before reads no id from a hit or a looked-up title, and every look-up it sends is refused for a missing id; it must regenerate and send and read `ExternalId`s. The web app is the only client.

## 2026-11-12, #727

- None that `buf breaking` reports: `nama/v1/plugin.proto`'s `PluginSummary.logo_svg` (10) is deleted with its number and name reserved.
- A Plugin's logo is no longer sent as SVG text. `PluginSummary.logo_url` (11) is its URL under `/media/`, its Custom Image's or its Manifest's, and `custom_logo` (12) says whether it is a Custom Image.
- A client built before still decodes a Plugin, and finds no logo; once it regenerates it shows the logo from `logo_url`, as an image. The web app is the only client. No Installed Plugin sees `PluginSummary`, and a Manifest's `logo_svg` is unchanged.

## 2026-11-11, #689

- `FIELD_NO_DELETE_UNLESS_NAME_RESERVED`, `nama/v1/playback.proto`: `PlaybackPlan.reasons` (4); `nama/v1/event.proto`: `Playback.reasons` (6).
- A plan's and a playback's reasons are `PlaybackReason`s, each a `PlaybackReasonKind` with its `params`, the Server's English `text` and whether it `decided` the plan (ADR 0034), under the new numbers 13 and 7. The old numbers are reserved, and the names kept for the new fields.
- A client built before still decodes a plan and a playback, and finds no reasons; once it regenerates it reads each reason's kind and params, or shows its `text` for a kind it doesn't know. No Installed Plugin sees either.

## 2026-11-11, #623

- `FIELD_SAME_TYPE`, `nama/v1/request.proto`: `SearchMoviesResponse.movies` (1) and `SearchShowsResponse.shows` (1).
- A search now gives a `MovieHit` or a `ShowHit`, which hold only what a search fills; `LookUpMovie` and `LookUpShow` still give a `FoundMovie` and a `FoundShow`. Each hit keeps the numbers its fields had in the message it replaces, so the bytes and the JSON are unchanged.
- A client built before still decodes a search; once it regenerates it reads a hit as a `MovieHit` or a `ShowHit`. No Installed Plugin sees a search.

## 2026-11-11, #622

- `FIELD_SAME_ONEOF`, `nama/v1/request.proto`: `Request.movie` (2) and `Request.season` (7).
- `Request.movie` and `Request.season` are now the two arms of the oneof `movie_or_season`, keeping their names and numbers, so the bytes and the JSON are unchanged.
- A client or an Installed Plugin built before still decodes a Request; once it regenerates it reads what the Request asks for from `movie_or_season` instead of testing which field is set.

## 2026-11-11, #624

- `FIELD_SAME_TYPE`, `nama/v1/browse.proto`.
- `ContinueWatchingEntry.episode` (3) is now a `ContinueWatchingEpisode`, which holds the `Episode` and the `Show` it is of. `ContinueWatchingEntry.show` (4) is removed and reserved.
- A client built before misreads an Episode entry, or fails to decode the response, and finds no Show; it must regenerate and read both from the Episode arm. No Installed Plugin sees Continue Watching.

## 2026-11-11, #625

- `ENUM_NO_DELETE` and `MESSAGE_NO_DELETE`, `nama/plugin/v1/manifest.proto`: `SettingKind` and `SettingValue` are deleted.
- `FIELD_SAME_TYPE`, `nama/plugin/v1/manifest.proto`: `SettingField.kind` is now a `nama.v1.SettingKind` and `SettingField.default_value` a `nama.v1.SettingValue`.

A Plugin's Manifest declares its settings with the setting kind and value an Admin is shown, so the schema has one of each. The two numbered `URL` and `CHOICE` the other way round, and the values' fields differ in name and number, so the Nama API version is now 0.2.0, and a Server refuses a Plugin built for 0.1, naming the version it wants, rather than reading its URL settings as choices.

An Installed Plugin built before must be rebuilt against `nama-pdk` with `nama-proto` 0.2.0, taking `SettingKind` and `SettingValue` from `nama_proto::nama::v1` and writing a default's text as `setting_value::Value::TextValue` (and `BoolValue` and `IntegerValue`). Clients of `nama.v1` need do nothing.
