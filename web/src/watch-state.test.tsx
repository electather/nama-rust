import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";

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

test("Continue Watching lists what you started, and hides when there's nothing", async () => {
  const entries = [
    {
      libraryId: 1n,
      movieOrEpisode: {
        case: "movie",
        value: {
          id: { id: alien },
          title: "Alien",
          watchState: { positionMs: 1n, durationMs: 4n },
        },
      },
    },
    {
      libraryId: 2n,
      movieOrEpisode: {
        case: "episode",
        value: {
          episode: {
            id: { id: { case: "tmdb", value: 1n } },
            seasonNumber: 2,
            episodeNumber: 1,
            watchState: { positionMs: 1n, durationMs: 4n },
          },
          show: { id: { id: { case: "tmdb", value: 70523n } }, title: "Dark" },
        },
      },
    },
  ] as const;
  openApp("/", server({ listContinueWatching: () => ({ entries: [...entries] }) }));
  expect(await screen.findByRole("heading", { name: "Continue Watching" })).toBeDefined();
  expect(screen.getByText("Alien")).toBeDefined();
  expect(screen.getByText("Dark")).toBeDefined();
  cleanup();

  openApp("/", server({ listContinueWatching: () => ({ entries: [] }) }));
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
