import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  BrowseService,
  type ContinueWatchingEntrySchema,
  HdrFormat,
  type MovieSchema,
  type ShowSchema,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** A fake Server with a Movies Library (1) and a Shows Library (2), browsed through `browse`. */
function server(browse: Partial<ServiceImpl<typeof BrowseService>>) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, browse);
  });
}

/** Opens the page of `movie`, Parasite with one unprobed Version unless it says otherwise. */
function openMovie(movie: MessageInitShape<typeof MovieSchema>) {
  const parasite = { title: "Parasite", versions: [{ id: 1n, fileName: "Parasite.mkv" }] };
  openApp(
    "/libraries/1/movies/tmdb-496243",
    server({ getMovie: () => ({ movie: { ...parasite, ...movie } }) }),
  );
}

test("a Movie's page shows its Title Logo with its genres, year, runtime and Content Rating", async () => {
  openMovie({
    titleLogoUrl: "/media/artwork/tmdb/%2Fparasite-logo.png",
    backdropUrl: "/media/artwork/tmdb/%2Fparasite-backdrop.jpg",
    genres: ["Thriller", "Drama"],
    releaseDate: "2019-05-30",
    runtimeMinutes: 133,
    contentRating: "R",
  });

  const heading = await screen.findByRole("heading", { level: 1, name: "Parasite" });
  expect(within(heading).getByRole("img").getAttribute("src")).toBe(
    "/media/artwork/tmdb/%2Fparasite-logo.png",
  );
  expect(screen.getByText("Movie · Thriller · Drama · 2019 · 2h 13m")).toBeDefined();
  expect(screen.getByText("R")).toBeDefined();
});

test("a title without a Title Logo shows its title as text", async () => {
  openMovie({});

  const heading = await screen.findByRole("heading", { level: 1, name: "Parasite" });
  expect(within(heading).queryByRole("img")).toBeNull();
});

test("a Movie shows 4K and HDR from one Version, and SDH from another's subtitles", async () => {
  openMovie({
    versions: [
      {
        id: 1n,
        size: 60_000_000_000n,
        tracks: [
          { kind: { case: "video", value: { width: 3840, height: 1600, hdr: HdrFormat.HDR10 } } },
        ],
      },
      {
        id: 2n,
        size: 2_000_000_000n,
        tracks: [
          { kind: { case: "video", value: { width: 1920, height: 800 } } },
          { sdh: true, kind: { case: "subtitle", value: {} } },
        ],
      },
    ],
  });

  expect(await screen.findByText("4K")).toBeDefined();
  expect(screen.getByText("HDR")).toBeDefined();
  expect(screen.getByText("SDH")).toBeDefined();
});

test("Dolby Vision is named as such, and a Movie without 4K, HDR or SDH has no badges", async () => {
  openMovie({
    versions: [
      {
        id: 1n,
        tracks: [
          {
            kind: {
              case: "video",
              value: { width: 1920, height: 1080, hdr: HdrFormat.DOLBY_VISION },
            },
          },
          { kind: { case: "subtitle", value: {} } },
        ],
      },
    ],
  });

  expect(await screen.findByText("Dolby Vision")).toBeDefined();
  expect(screen.queryByText(/^(4K|HDR|SDH)$/u)).toBeNull();
});

test("a started Movie shows Resume with its progress and time left", async () => {
  openMovie({ watchState: { positionMs: 1_800_000n, durationMs: 6_120_000n } });

  const resume = await screen.findByRole("link", { name: "Resume" });
  expect(resume.getAttribute("href")).toBe("/libraries/1/movies/tmdb-496243/play?version=1");
  expect(screen.getByRole("progressbar", { name: "Watched" })).toBeDefined();
  expect(screen.getByText("1h 12m left")).toBeDefined();
});

test("an unstarted Movie shows Play, and a played one too", async () => {
  openMovie({ watchState: { positionMs: 0n, durationMs: 6_120_000n, played: true } });

  expect(await screen.findByRole("link", { name: "Play" })).toBeDefined();
  expect(screen.queryByRole("progressbar")).toBeNull();
});

const dark = { id: { id: { case: "tmdb", value: 70_523n } }, title: "Dark" } as const;

/** Opens Dark's page, with `entries` in Continue Watching. */
function openDark(...entries: MessageInitShape<typeof ContinueWatchingEntrySchema>[]) {
  const show = {
    ...dark,
    overview: "A missing child sets four families on a hunt.",
    firstAirDate: "2017-12-01",
    genres: ["Drama"],
  };
  const transport = server({
    getShow: () => ({ show, seasons: [{ number: 1 }] }),
    listEpisodes: () => ({ episodes: [] }),
    listContinueWatching: () => ({ entries }),
  });
  openApp("/libraries/2/shows/tmdb-70523", transport);
}

/** A Continue Watching entry for Season 2's Episode `episodeNumber` of `show`, 10 minutes in. */
function episodeEntry(
  show: MessageInitShape<typeof ShowSchema>,
  libraryId: bigint,
  episodeNumber: number,
) {
  const watchState = { positionMs: 600_000n, durationMs: 2_580_000n };
  return {
    libraryId,
    movieOrEpisode: {
      case: "episode",
      value: { show, episode: { seasonNumber: 2, episodeNumber, watchState } },
    },
  } as const;
}

test("a Show's page shows its own overview and first air year, and no button unstarted", async () => {
  openDark();

  expect(await screen.findByText("A missing child sets four families on a hunt.")).toBeDefined();
  expect(screen.getByText("Show · Drama · 2017")).toBeDefined();
  expect(screen.queryByRole("link", { name: "Resume" })).toBeNull();
});

test("a Show's page resumes its most recently started Episode in Continue Watching", async () => {
  const other = { id: { id: { case: "tmdb", value: 1n } }, title: "Other" } as const;
  // Most recent first: another Show's, then Dark's in another Library, then this one's.
  openDark(
    episodeEntry(other, 2n, 1),
    episodeEntry(dark, 3n, 2),
    episodeEntry(dark, 2n, 3),
    episodeEntry(dark, 2n, 4),
  );

  const resume = await screen.findByRole("link", { name: "Resume" });
  expect(resume.getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-70523/seasons/2/episodes/3/play",
  );
  expect(screen.getByText("S2, E3 · 33m left")).toBeDefined();
});

test("an overview longer than two lines opens with More, and closes with Less", async () => {
  // happy-dom lays nothing out, so the overview is made taller than its two lines here.
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get: () => 96,
  });
  try {
    openMovie({ overview: "Greed and class discrimination threaten a newly formed bond." });

    fireEvent.click(await screen.findByRole("button", { name: "More" }));

    expect(screen.getByRole("button", { name: "Less" }).getAttribute("aria-expanded")).toBe("true");
  } finally {
    // Back to happy-dom's own getter, on `Element`.
    Reflect.deleteProperty(HTMLElement.prototype, "scrollHeight");
  }
});
