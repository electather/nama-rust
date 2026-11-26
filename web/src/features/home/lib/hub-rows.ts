import type { Transport } from "@connectrpc/connect";

import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import {
  type HomeRow,
  KernelHomeRow,
  type PluginHomeRow,
  type RecentlyAddedHomeRow,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

/** One of the Account's Servers the page at the Hub is signed in to. */
export interface ReachedServer {
  server: LinkedServer;
  transport: Transport;
}

/** A Home row as one Server gives it. */
export type RowOn<Row> = ReachedServer & { row: Row };

/** One row of Home at the Hub: a kind of row, merged from each Server that has it. */
type HubRow =
  | { case: "continueWatching"; key: string }
  | { case: "plugin"; key: string; heading: string; sources: RowOn<PluginHomeRow>[] }
  | { case: "recentlyAdded"; key: string; heading: string; sources: RowOn<RecentlyAddedHomeRow>[] };

/**
 * Each Server's Home rows merged by kind, in the order each kind first comes, Servers in turn:
 * Continue Watching, each Plugin's row by its Plugin and key, and Recently Added by Library kind.
 * A Kernel row this app doesn't know is left out.
 */
export function mergeRows(servers: (ReachedServer & { rows: HomeRow[] })[]) {
  const merged = new Map<string, HubRow>();
  for (const { rows, ...server } of servers) {
    for (const { kernelOrPlugin: row } of rows) {
      if (row.case === "plugin") {
        addPluginRow(merged, { ...server, row: row.value });
      } else if (row.case === "recentlyAdded") {
        addRecentlyAdded(merged, { ...server, row: row.value });
      } else if (row.value === KernelHomeRow.CONTINUE_WATCHING) {
        merged.set("continue-watching", { case: "continueWatching", key: "continue-watching" });
      }
    }
  }
  return [...merged.values()];
}

function addPluginRow(merged: Map<string, HubRow>, source: RowOn<PluginHomeRow>) {
  const key = `plugin/${source.row.pluginId}/${source.row.key}`;
  const kept = merged.get(key);
  if (kept?.case === "plugin") {
    kept.sources.push(source);
  } else {
    merged.set(key, { case: "plugin", key, heading: source.row.heading, sources: [source] });
  }
}

function addRecentlyAdded(merged: Map<string, HubRow>, source: RowOn<RecentlyAddedHomeRow>) {
  const shows = source.row.libraryKind === LibraryKind.SHOWS;
  const key = shows ? "recently-added/shows" : "recently-added/movies";
  const kept = merged.get(key);
  if (kept?.case === "recentlyAdded") {
    kept.sources.push(source);
  } else {
    const heading = shows ? "Recently added Shows" : "Recently added Movies";
    merged.set(key, { case: "recentlyAdded", key, heading, sources: [source] });
  }
}
