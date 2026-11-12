import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, HdrFormat, type MovieSchema } from "@/gen/nama/v1/browse_pb";
import {
  EventService,
  type SubscribeResponse,
  SubscribeResponseSchema,
} from "@/gen/nama/v1/event_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

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
    router.service(BrowseService, browse);
    if (events) router.service(EventService, { subscribe: () => events });
  });
}

test("an empty Library says so", async () => {
  openApp("/libraries/1", serverToBrowse({ listMovies: () => ({ movies: [] }) }));

  expect(await screen.findByRole("heading", { name: "Films" })).toBeDefined();
  expect(screen.getByText("Nothing in this Library yet")).toBeDefined();
});

test("an unknown Movie shows the not-found page, under the app chrome", async () => {
  const transport = serverToBrowse({
    getMovie: () => {
      throw new ConnectError("no such Movie", Code.NotFound);
    },
  });
  openApp("/libraries/1/movies/tmdb-404", transport);

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
  expect(screen.getByRole("link", { name: "Libraries" })).toBeDefined();
});

test("a Season the Show doesn't have falls back to its first, after the Specials", async () => {
  const transport = serverToBrowse({
    getShow: () => ({
      show: { title: "Severance" },
      seasons: [{ number: 0 }, { number: 1 }, { number: 2 }],
    }),
    listEpisodes: ({ seasonNumber }) => ({
      episodes: [
        {
          seasonNumber,
          episodeNumber: 1,
          title: `Opener of Season ${seasonNumber}`,
          versions: [{ id: 1n }],
        },
      ],
    }),
  });
  openApp("/libraries/2/shows/tmdb-95396?season=9", transport);

  expect(await screen.findByText(/Opener of Season 1/u)).toBeDefined();
  expect(screen.queryByText(/Opener of Season (0|9)/u)).toBeNull();
});

test("a Library's page shows a Movie as soon as it's Matched", async () => {
  const listed: MessageInitShape<typeof MovieSchema>[] = [];
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

  listed.push({ id: { id: { case: "tmdb", value: 348n } }, title: "Alien" });
  match?.();

  expect(await screen.findByText("Alien")).toBeDefined();
});

test("a Movie's Version choice says each one's picture and sound, such as 4K, HDR and 5.1", async () => {
  const transport = serverToBrowse({
    getMovie: () => ({
      movie: {
        title: "Alien",
        releaseDate: "1979-05-25",
        versions: [
          {
            id: 1n,
            fileName: "Alien (1979) - 2160p.mkv",
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
            fileName: "Alien (1979) - 720p.mkv",
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

  expect(
    await screen.findByRole("option", {
      name: "Alien (1979) - 2160p.mkv (4K · HDR · 5.1 · 60 GB)",
    }),
  ).toBeDefined();
  expect(
    screen.getByRole("option", { name: "Alien (1979) - 720p.mkv (720p · Stereo · 2 GB)" }),
  ).toBeDefined();
  expect(screen.getByRole("option", { name: "Alien (1979).avi (0.7 GB)" })).toBeDefined();
});

test("a Show's Episodes each say the picture and sound of the Version they play", async () => {
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

  expect(await screen.findByText("Feb 18, 2022 · 1080p · 7.1")).toBeDefined();
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

  const episode = await screen.findByRole("link", { name: /In Perpetuity/u });
  expect(episode.getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-95396/seasons/1/episodes/3/play",
  );
  expect(episode.querySelector("img")?.getAttribute("src")).toBe(still);
  expect(within(episode).getByText("Episode 3")).toBeDefined();
  expect(within(episode).getByText("Mark is promoted.")).toBeDefined();
  expect(within(episode).getByText("55 min")).toBeDefined();
  expect(within(episode).getByRole("progressbar", { name: "Watched" })).toBeDefined();
});

test("a Special without a Still shows the placeholder, named as a Special", async () => {
  openApp("/libraries/2/shows/tmdb-95396", serverWithEpisode(0, "Behind the Scenes"));

  const special = await screen.findByRole("link", { name: /Behind the Scenes/u });
  expect(special.querySelector("img")).toBeNull();
  expect(within(special).getByText("Specials, Episode 3")).toBeDefined();
});
