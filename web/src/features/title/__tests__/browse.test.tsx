import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService, HdrFormat, type MovieSummarySchema } from "@/gen/nama/v1/browse_pb";
import {
  EventService,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { externalIdFromParam } from "@/shared/hooks/browse";
import { titleParam } from "@/shared/lib/title-param";
import { fakeServer, newUser, openApp } from "@/test-app";
import { layOutShelves } from "@/test-layout";
import { openMenu } from "@/test-menu";

// A Show's row of Episodes is virtual, so it needs a layout to show its cards.
beforeEach(layOutShelves);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** A fake Server with a Movies Library (1) and a Shows Library (2), browsed through `browse`. */
function serverToBrowse(
  browse: Partial<ServiceImpl<typeof BrowseService>>,
  events?: AsyncIterable<SubscribeResponse>,
) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, { listGenres: () => ({}), listUnmatched: () => ({}), ...browse });
    if (events) {
      router.service(EventService, { subscribe: () => events });
    }
  });
}

test("a title's id round-trips through its URL form, the value percent-encoded", () => {
  for (const [param, namespace, value] of [
    ["tmdb-603", "tmdb", "603"],
    ["anidb-a%2Fb", "anidb", "a/b"],
  ] as const) {
    const id = externalIdFromParam(param);
    expect([id.namespace, id.value]).toStrictEqual([namespace, value]);
    expect(titleParam(id)).toBe(param);
  }
});

test("a malformed title id in the URL is not found", () => {
  for (const param of ["TMDB-1", "tmdb-", "tmdb-%"]) {
    expect(() => externalIdFromParam(param)).toThrow(expect.objectContaining({ isNotFound: true }));
  }
});

test("an empty Library says so", async () => {
  openApp("/libraries/1", serverToBrowse({ listMovies: () => ({ movies: [] }) }));

  await expect(screen.findByRole("heading", { name: "Films" })).resolves.toBeDefined();
  expect(screen.getByText("Nothing in this Library yet")).toBeDefined();
});

test("an unknown Movie shows the not-found page, under the app chrome", async () => {
  const transport = serverToBrowse({
    getMovie: () => {
      throw new ConnectError("no such Movie", Code.NotFound);
    },
  });
  openApp("/libraries/1/movies/tmdb-404", transport);

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Home" })).toBeDefined();
});

/** Season `seasonNumber`'s one Episode, titled after it. */
function opener(seasonNumber: number) {
  return {
    seasonNumber,
    episodeNumber: 1,
    title: `Opener of Season ${seasonNumber}`,
    versions: [{ id: 1n }],
  };
}

/** A fake Server with a Show of `seasons`, in the Server's order, each holding its `opener`. */
function serverWithSeasons(...seasons: number[]) {
  return serverToBrowse({
    getShow: () => ({
      show: { title: "Severance" },
      seasons: seasons.map((number) => ({ number })),
    }),
    listEpisodes: () => ({
      episodes: seasons.map((number) => opener(number)),
    }),
  });
}

/** The titles of the Episodes in the Show's row, in order. */
function rowTitles() {
  const row = screen.getByRole("list", { name: "Episodes" });
  return within(row)
    .getAllByRole("listitem")
    .map((card) => /Opener of Season \d/u.exec(card.textContent)?.[0]);
}

test("a Show lists every Season's Episodes in one row, Specials last, under the Season it's on", async () => {
  openApp("/libraries/2/shows/tmdb-95396", serverWithSeasons(0, 1, 2));

  await expect(screen.findByText("Opener of Season 2")).resolves.toBeDefined();
  expect(rowTitles()).toStrictEqual([
    "Opener of Season 1",
    "Opener of Season 2",
    "Opener of Season 0",
  ]);
  // It opens on the Season of the Episode Play starts, and its menu picks any other.
  await openMenu("Season 1");
  const seasons = await screen.findAllByRole("menuitem");
  expect(seasons.map((season) => season.textContent)).toStrictEqual([
    "Season 1",
    "Season 2",
    "Specials",
  ]);
  fireEvent.click(screen.getByRole("menuitem", { name: "Specials" }));

  await expect(screen.findByRole("button", { name: "Specials" })).resolves.toBeDefined();
});

test("the row opens on the Season asked for, or the first for one the Show doesn't have", async () => {
  openApp("/libraries/2/shows/tmdb-95396?season=2", serverWithSeasons(0, 1, 2));
  await screen.findByText("Opener of Season 2");
  expect(screen.getByRole("button", { name: "Season 2" })).toBeDefined();
  cleanup();

  openApp("/libraries/2/shows/tmdb-95396?season=9", serverWithSeasons(0, 1, 2));
  await screen.findByText("Opener of Season 2");
  expect(screen.getByRole("button", { name: "Season 1" })).toBeDefined();
});

test("a Show's page asks for every Season's Episodes once, alongside the Show, however many Seasons", async () => {
  for (const count of [1, 20]) {
    const seasons = Array.from({ length: count }, (_, index) => index + 1);
    let askForEpisodes: (() => void) | undefined;
    const episodesAsked = new Promise<void>((resolve) => {
      askForEpisodes = resolve;
    });
    const asked: (number | undefined)[] = [];
    const transport = serverToBrowse({
      // It answers once the Episodes are asked for, so a page waiting for it never opens.
      getShow: async () => {
        await episodesAsked;
        return { show: { title: "Severance" }, seasons: seasons.map((number) => ({ number })) };
      },
      listEpisodes: ({ seasonNumber }) => {
        asked.push(seasonNumber);
        askForEpisodes?.();
        return { episodes: seasons.map((number) => opener(number)) };
      },
    });
    openApp("/libraries/2/shows/tmdb-95396", transport);

    // oxlint-disable-next-line no-await-in-loop -- each Show's page opens in turn
    await screen.findByText("Opener of Season 1");
    expect(asked).toStrictEqual([undefined]);
    cleanup();
  }
});

test("a Show with one Season names it over its Episodes, with no menu to pick another", async () => {
  openApp("/libraries/2/shows/tmdb-95396", serverWithSeasons(3));

  await expect(screen.findByText("Opener of Season 3")).resolves.toBeDefined();
  expect(screen.getByRole("heading", { name: "Season 3" })).toBeDefined();
  expect(screen.queryByRole("button", { name: "Season 3" })).toBeNull();
});

test("a Library's page shows a Movie as soon as it's Matched", async () => {
  const listed: MessageInitShape<typeof MovieSummarySchema>[] = [];
  let match: (() => void) | undefined;
  const matched = new Promise<void>((resolve) => {
    match = resolve;
  });
  async function* events() {
    await matched;
    yield create(SubscribeResponseSchema, {
      message: {
        case: "event",
        value: { event: { case: "itemMatched", value: { libraryId: 1n } } },
      },
    });
  }
  openApp("/libraries/1", serverToBrowse({ listMovies: () => ({ movies: listed }) }, events()));
  await screen.findByText("Nothing in this Library yet");

  listed.push({ id: { namespace: "tmdb", value: "348" }, title: "Alien" });
  match?.();

  await expect(screen.findByText("Alien")).resolves.toBeDefined();
});

test("a Movie's menu chooses its Version, each by its Edition, picture, sound and size, the largest first", async () => {
  const transport = serverToBrowse({
    getMovie: () => ({
      movie: {
        title: "Alien",
        releaseDate: "1979-05-25",
        versions: [
          {
            id: 1n,
            fileName: "Alien (1979) {edition-Director's Cut} - 2160p.mkv",
            edition: "Director's Cut",
            size: 60_000_000_000n,
            tracks: [
              {
                codec: "hevc",
                kind: {
                  case: "video",
                  value: { width: 3840, height: 1600, bitDepth: 10, hdr: HdrFormat.HDR10 },
                },
              },
              { codec: "ac3", kind: { case: "audio", value: { channels: 2 } } },
              { codec: "truehd", default: true, kind: { case: "audio", value: { channels: 6 } } },
            ],
          },
          {
            id: 2n,
            fileName: "Alien (1979) {edition-Director's Cut} - 720p.mkv",
            edition: "Director's Cut",
            size: 2_000_000_000n,
            tracks: [
              { codec: "h264", kind: { case: "video", value: { width: 1280, height: 534 } } },
              { codec: "aac", kind: { case: "audio", value: { channels: 2 } } },
            ],
          },
          // Not probed yet.
          { id: 3n, fileName: "Alien (1979).avi", size: 700_000_000n },
        ],
      },
    }),
  });
  openApp("/libraries/1/movies/tmdb-348", transport);

  await openMenu("More actions");
  const versions = await screen.findAllByRole("menuitemradio");
  expect(
    versions.map((version) => [version.textContent, version.getAttribute("aria-checked")]),
  ).toStrictEqual([
    ["Director's Cut · 4K · HDR · 5.1 · 60 GB", "true"],
    ["Director's Cut · 720p · Stereo · 2 GB", "false"],
    ["700 MB", "false"],
  ]);
});

test("a Show's Details say the picture of its Episodes' Versions", async () => {
  const transport = serverToBrowse({
    getShow: () => ({ show: { title: "Severance" }, seasons: [{ number: 1 }] }),
    listEpisodes: () => ({
      episodes: [
        {
          seasonNumber: 1,
          episodeNumber: 1,
          title: "Good News About Hell",
          airDate: "2022-02-18",
          versions: [
            {
              id: 1n,
              tracks: [
                { kind: { case: "video", value: { width: 1920, height: 1080 } } },
                { kind: { case: "audio", value: { channels: 8 } } },
              ],
            },
          ],
        },
      ],
    }),
  });
  openApp("/libraries/2/shows/tmdb-95396", transport);

  const details = await screen.findByRole("region", { name: "Details" });
  expect(within(details).getByText("1080p")).toBeDefined();
});

/** A fake Server with a Show whose only Season is `seasonNumber`, holding Episode 3, `title`, half watched, with `stillUrl`. */
function serverWithEpisode(seasonNumber: number, title: string, stillUrl?: string) {
  return serverToBrowse({
    getShow: () => ({ show: { title: "Severance" }, seasons: [{ number: seasonNumber }] }),
    listEpisodes: () => ({
      episodes: [
        {
          seasonNumber,
          episodeNumber: 3,
          title,
          overview: "Mark is promoted.",
          runtimeMinutes: 55,
          stillUrl,
          versions: [{ id: 1n }],
          watchState: { positionMs: 1_650_000n, durationMs: 3_300_000n },
        },
      ],
    }),
  });
}

test("a Show's Episode is a card with its Still, number, synopsis, runtime and progress", async () => {
  const still = "/media/stills/1.jpg";
  openApp("/libraries/2/shows/tmdb-95396", serverWithEpisode(1, "In Perpetuity", still));

  const episode = await screen.findByRole("link", {
    name: "Resume Season 1, Episode 3: In Perpetuity, 28m left",
  });
  expect(episode.getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-95396/seasons/1/episodes/3/play",
  );
  const card = episode.closest("li");
  expect(card?.querySelector("img")?.getAttribute("src")).toBe(still);
  expect(card?.textContent).toContain("Episode 3");
  expect(card?.textContent).toContain("Mark is promoted.");
  expect(card?.textContent).toContain("28m");
});

test("a Special without a Still shows the placeholder, named as a Special", async () => {
  openApp("/libraries/2/shows/tmdb-95396", serverWithEpisode(0, "Behind the Scenes"));

  const special = await screen.findByRole("link", { name: /Behind the Scenes/u });
  const card = special.closest("li");
  expect(card?.querySelector("img")).toBeNull();
  expect(card?.textContent).toContain("Specials, Episode 3");
});
