import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  BrowseService,
  type ContinueWatchingEntrySchema,
  type ShowSchema,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { WatchStateService, type WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const alien = { case: "tmdb", value: 348n } as const;

/** A fake Server with a Movies Library (1) and a Shows Library (2). */
function server(
  browse: Partial<ServiceImpl<typeof BrowseService>>,
  watchState: Partial<ServiceImpl<typeof WatchStateService>> = {},
) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, browse);
    router.service(WatchStateService, watchState);
  });
}

/** The Home page, with `entries` in Continue Watching. */
function openHome(...entries: MessageInitShape<typeof ContinueWatchingEntrySchema>[]) {
  openApp("/", server({ listContinueWatching: () => ({ entries }) }));
}

/** A Continue Watching entry for Season `seasonNumber`'s Episode `episodeNumber` of `show`, in TV (2). */
function episodeEntry(
  show: MessageInitShape<typeof ShowSchema>,
  seasonNumber: number,
  episodeNumber: number,
  watchState: MessageInitShape<typeof WatchStateSchema>,
) {
  return {
    libraryId: 2n,
    movieOrEpisode: {
      case: "episode",
      value: { episode: { seasonNumber, episodeNumber, watchState }, show },
    },
  } as const;
}

test("a Movie's Continue Watching card resumes it, with how much is left", async () => {
  openHome({
    libraryId: 1n,
    movieOrEpisode: {
      case: "movie",
      value: {
        id: { id: alien },
        title: "Alien",
        backdropUrl: "/media/artwork/tmdb/%2Falien-backdrop.jpg",
        watchState: { positionMs: 1_800_000n, durationMs: 6_120_000n },
      },
    },
  });

  const card = await screen.findByRole("link", { name: /Alien/u });
  expect(card.getAttribute("href")).toBe("/libraries/1/movies/tmdb-348/play");
  expect(card.querySelector("img")?.getAttribute("src")).toBe(
    "/media/artwork/tmdb/%2Falien-backdrop.jpg",
  );
  expect(within(card).getByText("1h 12m left")).toBeDefined();
  expect(within(card).getByRole("progressbar", { name: "Watched" })).toBeDefined();
});

test("a Season 2 Episode's card shows its Show's Backdrop, not a Season's Poster", async () => {
  const dark = {
    id: { id: { case: "tmdb", value: 70523n } },
    title: "Dark",
    posterUrl: "/media/artwork/tmdb/%2Fdark-season-1.jpg",
    backdropUrl: "/media/artwork/tmdb/%2Fdark-backdrop.jpg",
  } as const;
  openHome(episodeEntry(dark, 2, 3, { positionMs: 1_020_000n, durationMs: 3_000_000n }));

  const card = await screen.findByRole("link", { name: /Dark/u });
  expect(card.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523/seasons/2/episodes/3/play");
  expect(card.querySelector("img")?.getAttribute("src")).toBe(
    "/media/artwork/tmdb/%2Fdark-backdrop.jpg",
  );
  expect(within(card).getByText("S2, E3 · 33m left")).toBeDefined();
  expect(within(card).getByRole("progressbar", { name: "Watched" })).toBeDefined();
});

test("a Special without a Backdrop shows the placeholder with its Show's title", async () => {
  const severance = { id: { id: { case: "tmdb", value: 95396n } }, title: "Severance" } as const;
  openHome(episodeEntry(severance, 0, 1, { positionMs: 1n, durationMs: 60_000n }));

  const card = await screen.findByRole("link", { name: /Severance/u });
  expect(card.querySelector("img")).toBeNull();
  expect(within(card).getByText("Specials, E1 · 1m left")).toBeDefined();
  expect(within(card).getByRole("progressbar", { name: "Watched" })).toBeDefined();
});

test("Continue Watching hides when there's nothing", async () => {
  openHome();
  expect(await screen.findByRole("heading", { name: "Libraries" })).toBeDefined();
  expect(screen.queryByRole("heading", { name: "Continue Watching" })).toBeNull();
});

test("marking a Season played marks each of its Episodes played", async () => {
  let played = false;
  let marked: string | undefined;
  const transport = server(
    {
      getShow: () => ({ show: { title: "Severance" }, seasons: [{ number: 1 }] }),
      listEpisodes: () => ({
        episodes: [1, 2].map((episodeNumber) => ({
          id: { id: { case: "tmdb", value: BigInt(episodeNumber) } },
          seasonNumber: 1,
          episodeNumber,
          title: `Episode ${episodeNumber}`,
          versions: [{ id: BigInt(episodeNumber) }],
          watchState: { played },
        })),
      }),
    },
    {
      setPlayed: (request) => {
        marked = request.target.case;
        ({ played } = request);
        return {};
      },
    },
  );
  openApp("/libraries/2/shows/tmdb-95396", transport);
  const marks = await screen.findAllByRole("button", { name: /^Played: Episode/u });
  expect(marks.map((mark) => mark.getAttribute("aria-pressed"))).toEqual(["false", "false"]);

  fireEvent.click(screen.getByRole("button", { name: "More for this Show" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Mark Season 1 played" }));

  await waitFor(() => {
    const now = screen.getAllByRole("button", { name: /^Played: Episode/u });
    expect(now.map((mark) => mark.getAttribute("aria-pressed"))).toEqual(["true", "true"]);
  });
  expect(marked).toBe("season");
});
