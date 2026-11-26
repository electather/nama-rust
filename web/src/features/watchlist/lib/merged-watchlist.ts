import type { Transport } from "@connectrpc/connect";

import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import type { HomeRowTitle } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { WatchlistTitle } from "@/gen/nama/v1/watchlist_pb";
import type { RowTitle } from "@/shared/lib/row-titles";
import { titleParam } from "@/shared/lib/title-param";

/** One Server's Watchlist, `server` unset for the Server that served the page. */
export interface ServerWatchlist {
  transport: Transport;
  server: LinkedServer | undefined;
  titles: WatchlistTitle[];
}

/** A title as one Server's Watchlist keeps it: the id `RemoveFromWatchlist` takes it off by. */
export interface Listed {
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
  listed: Listed[];
}

/**
 * `watchlists` merged, each title once by its kind and the External ID it shows by, in the order
 * first listed. It shows as the first Server with it in a Library has it, else as the first lists
 * it.
 */
export function mergedWatchlist(watchlists: ServerWatchlist[]): WatchlistEntry[] {
  // shortcut: a Server listing a title by an External ID another doesn't show it by lists it
  // twice; match on every id a title has once the Server sends them.
  const entries = new Map<string, WatchlistEntry>();
  for (const watchlist of watchlists) {
    for (const title of watchlist.titles) {
      addListed(entries, watchlist, title);
    }
  }
  return [...entries.values()];
}

/** Adds `title`, as `watchlist`'s Server lists it, to `entries`, unless it shows as nothing. */
function addListed(
  entries: Map<string, WatchlistEntry>,
  { transport, server }: ServerWatchlist,
  { id, title }: WatchlistTitle,
) {
  const shown = rowTitleOf(title);
  if (id !== undefined && shown !== undefined) {
    add(entries, { transport, kind: kindOf(shown), id }, { title: shown, server, transport });
  }
}

/** How `title` shows, unless it's a kind this app doesn't know. */
function rowTitleOf(title: HomeRowTitle | undefined): RowTitle | undefined {
  const shown = title?.movieOrShow;
  return shown?.case === undefined ? undefined : shown;
}

/** Adds the title `shown` on a Server, `listed` there, to `entries`, which may have it already. */
function add(
  entries: Map<string, WatchlistEntry>,
  listed: Listed,
  shown: Pick<WatchlistEntry, "title" | "server" | "transport">,
) {
  const key = `${listed.kind}/${titleParam(shown.title.value.id)}`;
  const entry = entries.get(key);
  if (entry === undefined) {
    entries.set(key, { key, ...shown, listed: [listed] });
    return;
  }
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
