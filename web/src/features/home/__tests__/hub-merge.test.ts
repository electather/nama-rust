import { create } from "@bufbuild/protobuf";
import { createRouterTransport } from "@connectrpc/connect";
import { expect, test } from "vite-plus/test";

import { entryCard } from "@/features/home/lib/continue-watching-cards";
import { type ReachedServer, mergeRows } from "@/features/home/lib/hub-rows";
import { mergeContinueWatching, mergeTitles } from "@/features/home/lib/hub-titles";
import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import {
  ContinueWatchingEntrySchema,
  HomeRowSchema,
  MovieInLibrarySchema,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { MovieHitSchema } from "@/gen/nama/v1/request_pb";
import { type HomeRowInit, pluginRow, recentlyAddedRow } from "@/test-home-rows";

function reached(id: string): ReachedServer {
  return { server: create(LinkedServerSchema, { id }), transport: createRouterTransport(() => {}) };
}

const basement = reached("basement");
const cabin = reached("cabin");
const alien = { namespace: "tmdb", value: "348" };

/** `server`'s Continue Watching card for the Movie `title`, last watched at `updatedAt`. */
function movieCard(server: ReachedServer, title: string, updatedAt: bigint) {
  const value = { id: { namespace: "tmdb", value: title }, title, watchState: { updatedAt } };
  const entry = create(ContinueWatchingEntrySchema, {
    libraryId: 1n,
    movieOrEpisode: { case: "movie", value },
  });
  return entryCard(entry).map((card) => ({
    server: server.server,
    transport: server.transport,
    card,
  }));
}

/** `server`'s Continue Watching card for the next Episode of the Show `title`. */
function nextEpisodeCard(server: ReachedServer, title: string) {
  const show = { id: { namespace: "tmdb", value: title }, title };
  const episode = { id: { namespace: "tmdb", value: `${title}-1-2` } };
  const entry = create(ContinueWatchingEntrySchema, {
    libraryId: 2n,
    movieOrEpisode: { case: "episode", value: { show, episode } },
  });
  return entryCard(entry).map((card) => ({
    server: server.server,
    transport: server.transport,
    card,
  }));
}

function homeRows(...rows: HomeRowInit[]) {
  return rows.map((row) => create(HomeRowSchema, row));
}

test("a Show's next Episode, which has no Watch State, keeps its place after the card above it", () => {
  const merged = mergeContinueWatching([
    [
      ...movieCard(basement, "Alien", 300n),
      ...nextEpisodeCard(basement, "Severance"),
      ...movieCard(basement, "Heat", 100n),
    ],
    movieCard(cabin, "Dune", 200n),
  ]);

  expect(merged.map(({ card }) => card.backdrop.title)).toStrictEqual([
    "Alien",
    "Severance",
    "Dune",
    "Heat",
  ]);
});

test("a title on one Server and missing from another is listed as on the Server", () => {
  const missing = create(MovieHitSchema, { id: alien, title: "Alien" });
  const present = create(MovieInLibrarySchema, { libraryId: 1n, id: alien, title: "Alien" });

  const merged = mergeTitles([
    [{ ...basement, title: { case: "missingMovie", value: missing } }],
    [{ ...cabin, title: { case: "movie", value: present } }],
  ]);

  expect(merged.map(({ server, title }) => [server.id, title.case])).toStrictEqual([
    ["cabin", "movie"],
  ]);
});

test("rows merge by kind, in the order each kind first comes", () => {
  const trending = pluginRow("discover", "trending", "Trending");
  const merged = mergeRows([
    { ...basement, rows: homeRows(trending, recentlyAddedRow(1n, "Films", LibraryKind.MOVIES)) },
    { ...cabin, rows: homeRows(recentlyAddedRow(2n, "TV", LibraryKind.SHOWS), trending) },
  ]);

  expect(merged.map(({ key }) => key)).toStrictEqual([
    "plugin/discover/trending",
    "recently-added/movies",
    "recently-added/shows",
  ]);
});
