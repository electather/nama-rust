import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  BrowseService,
  type ContinueWatchingEntrySchema,
  type ListContinueWatchingRequest,
  type ShowSchema,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { WatchStateService, type WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const alien = { namespace: "tmdb", value: "348" } as const;

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

/**
 * A Continue Watching entry for Season `seasonNumber`'s Episode `episodeNumber` of `show`, in TV
 * (2), without a `watchState` when it's the Show's next Episode.
 */
// oxlint-disable-next-line max-params -- an entry reads as its Show, Season, Episode and Watch State, in that order
function episodeEntry(
  show: MessageInitShape<typeof ShowSchema>,
  seasonNumber: number,
  episodeNumber: number,
  watchState?: MessageInitShape<typeof WatchStateSchema>,
) {
  return {
    libraryId: 2n,
    movieOrEpisode: {
      case: "episode",
      value: { episode: { seasonNumber, episodeNumber, watchState }, show },
    },
  } as const;
}

test("home asks for one page of Continue Watching, a row's worth", async () => {
  const asked: ListContinueWatchingRequest[] = [];
  openApp(
    "/",
    server({
      listContinueWatching: (request) => {
        asked.push(request);
        return {};
      },
    }),
  );

  await screen.findByRole("heading", { name: "Libraries" });
  expect(asked).toMatchObject([{ page: { size: 6, token: "" } }]);
});

test("a Movie's Continue Watching card resumes it, with how much is left", async () => {
  openHome({
    libraryId: 1n,
    movieOrEpisode: {
      case: "movie",
      value: {
        id: alien,
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
    id: { namespace: "tmdb", value: "70523" },
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

test("a Show's next Episode's card says which it is, with no progress", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" } as const;
  openHome(episodeEntry(dark, 1, 4));

  const card = await screen.findByRole("link", { name: /Dark/u });
  expect(card.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523/seasons/1/episodes/4/play");
  expect(within(card).getByText("S1, E4")).toBeDefined();
  expect(within(card).queryByRole("progressbar")).toBeNull();
});

test("a Special without a Backdrop shows the placeholder with its Show's title", async () => {
  const severance = { id: { namespace: "tmdb", value: "95396" }, title: "Severance" } as const;
  openHome(episodeEntry(severance, 0, 1, { positionMs: 1n, durationMs: 60_000n }));

  const card = await screen.findByRole("link", { name: /Severance/u });
  expect(card.querySelector("img")).toBeNull();
  expect(within(card).getByText("Specials, E1 · 1m left")).toBeDefined();
  expect(within(card).getByRole("progressbar", { name: "Watched" })).toBeDefined();
});

/** Where the fake Server keeps `name`'s Title Logo. */
function logo(name: string) {
  return `/media/artwork/tmdb/%2F${name}-logo.png`;
}

test("continue Watching cards show each title's Title Logo in place of its title", async () => {
  const dark = {
    id: { namespace: "tmdb", value: "70523" },
    title: "Dark",
    titleLogoUrl: logo("dark"),
  } as const;
  openHome(
    {
      libraryId: 1n,
      movieOrEpisode: {
        case: "movie",
        value: {
          id: alien,
          title: "Alien",
          titleLogoUrl: logo("alien"),
          watchState: { positionMs: 1n, durationMs: 60_000n },
        },
      },
    },
    episodeEntry(dark, 1, 1, { positionMs: 1n, durationMs: 60_000n }),
  );

  const alienCard = await screen.findByRole("link", { name: /Alien/u });
  expect(within(alienCard).getByRole("img", { name: "Alien" }).getAttribute("src")).toBe(
    logo("alien"),
  );
  const darkCard = screen.getByRole("link", { name: /Dark/u });
  expect(within(darkCard).getByRole("img", { name: "Dark" }).getAttribute("src")).toBe(
    logo("dark"),
  );
});

test("continue Watching hides when there's nothing", async () => {
  openHome();
  await expect(screen.findByRole("heading", { name: "Libraries" })).resolves.toBeDefined();
  expect(screen.queryByRole("heading", { name: "Continue Watching" })).toBeNull();
});

/**
 * A fake Server whose Continue Watching lists `entry` until it's hidden, with each hiding asked of
 * it: what it hid or showed again, and which.
 */
function serverHiding(entry: MessageInitShape<typeof ContinueWatchingEntrySchema>) {
  let hidden = false;
  const asked: { case: string | undefined; hidden: boolean }[] = [];
  const transport = server(
    { listContinueWatching: () => ({ entries: hidden ? [] : [entry] }) },
    {
      setHiddenFromContinueWatching: (request) => {
        ({ hidden } = request);
        asked.push({ case: request.target.case, hidden });
        return {};
      },
    },
  );
  return { transport, asked };
}

test("hiding a Show from Continue Watching takes its card away, and Undo brings it back", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
  const { transport, asked } = serverHiding(episodeEntry(dark, 1, 2));
  openApp("/", transport);

  fireEvent.click(await screen.findByRole("button", { name: "More for Dark" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Hide from Continue Watching" }));
  const undo = await screen.findByRole("button", { name: "Undo" });
  await waitFor(() => {
    expect(screen.queryByRole("link", { name: /Dark/u })).toBeNull();
  });
  expect(screen.getByRole("status").textContent).toContain(
    "Dark is hidden until you play it again.",
  );
  expect(document.activeElement).toBe(undo);

  fireEvent.click(undo);
  await expect(screen.findByRole("link", { name: /Dark/u })).resolves.toBeDefined();
  expect(asked).toStrictEqual([
    { case: "showId", hidden: true },
    { case: "showId", hidden: false },
  ]);
});
