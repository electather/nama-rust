import type { Transport } from "@connectrpc/connect";

import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import type { HomeRowTitle } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { WatchlistTitle } from "@/gen/nama/v1/watchlist_pb";
import type { RowTitle } from "@/shared/lib/row-titles";
import { titleParam } from "@/shared/lib/title-param";

/** One Server's Watchlist, `server` unset for the Server that served the page. */
export interface WatchlistOnServer {
  transport: Transport;
  server: LinkedServer | undefined;
  titles: WatchlistTitle[];
}

/** A title as one Server's Watchlist keeps it: the id `RemoveFromWatchlist` takes it off by. */
export interface Listing {
  transport: Transport;
  kind: TitleKind;
  id: ExternalId;
}

/**
 * A title on the Watchlist page: how it shows, on which Server and through its `transport`, and
 * each place it's listed.
 */
export interface WatchlistEntry {
  key: string;
  title: RowTitle;
  server: LinkedServer | undefined;
  transport: Transport;
  listed: Listing[];
}

/**
 * `watchlists` merged, each title once by its kind and either the External ID it's listed by or
 * the one it shows by, in the order first listed. It shows as the first Server with it in a
 * Library has it, else as the first lists it.
 */
export function mergedWatchlist(watchlists: WatchlistOnServer[]): WatchlistEntry[] {
  // Watchlists sync under the id a title was added by, while each Server shows it by its own
  // main id, so either id may be the one two Servers share.
  // ponytail: two Servers sharing neither id list it twice; match on every id a title has once
  // the Server sends them.
  const byId = new Map<string, WatchlistEntry>();
  for (const watchlist of watchlists) {
    for (const title of watchlist.titles) {
      addListed(byId, watchlist, title);
    }
  }
  // An entry's first id goes in as it's made, so the Set keeps the order first listed.
  return [...new Set(byId.values())];
}

/** Adds `title`, as `watchlist`'s Server lists it, to `byId`, unless it shows as nothing. */
function addListed(
  byId: Map<string, WatchlistEntry>,
  { transport, server }: WatchlistOnServer,
  { id, title }: WatchlistTitle,
) {
  const shown = rowTitleOf(title);
  if (id !== undefined && shown !== undefined) {
    add(byId, { transport, kind: kindOf(shown), id }, { title: shown, server, transport });
  }
}

/** How `title` shows, unless it's a kind this app doesn't know. */
function rowTitleOf(title: HomeRowTitle | undefined): RowTitle | undefined {
  const shown = title?.movieOrShow;
  return shown?.case === undefined ? undefined : shown;
}

/**
 * Adds the title `shown` on a Server, `listed` there, to `byId`, which may have it already under
 * either id.
 */
function add(
  byId: Map<string, WatchlistEntry>,
  listed: Listing,
  shown: Pick<WatchlistEntry, "title" | "server" | "transport">,
) {
  const key = `${listed.kind}/${titleParam(shown.title.value.id)}`;
  const listedKey = `${listed.kind}/${titleParam(listed.id)}`;
  const found = byId.get(key) ?? byId.get(listedKey);
  if (found !== undefined) {
    listAlso(found, listed, shown);
  }
  const entry = found ?? { key, ...shown, listed: [listed] };
  byId.set(key, entry);
  byId.set(listedKey, entry);
}

/** Adds `listed` to `entry`, showing it as `shown` if that has it in a Library and `entry` doesn't. */
function listAlso(
  entry: WatchlistEntry,
  listed: Listing,
  shown: Pick<WatchlistEntry, "title" | "server" | "transport">,
) {
  entry.listed.push(listed);
  if (isMissing(entry.title) && !isMissing(shown.title)) {
    Object.assign(entry, shown);
  }
}

function kindOf(title: RowTitle) {
  return title.case === "movie" || title.case === "missingMovie" ? TitleKind.MOVIE : TitleKind.SHOW;
}

function isMissing(title: RowTitle) {
  return title.case === "missingMovie" || title.case === "missingShow";
}
