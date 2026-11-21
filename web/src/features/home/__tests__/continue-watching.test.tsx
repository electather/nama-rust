import type { MessageInitShape } from "@bufbuild/protobuf";
import { type ServiceImpl, createRouterTransport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, renderHook, screen, waitFor } from "@testing-library/react";
import { type ReactNode, Suspense } from "react";
import { rgbaToThumbHash, thumbHashToDataURL } from "thumbhash";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import {
  continueWatchingQuery,
  useContinueWatching,
} from "@/features/home/data/use-continue-watching";
import {
  BrowseService,
  type ContinueWatchingEntrySchema,
  type ListContinueWatchingRequest,
  type ShowSchema,
} from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { WatchStateService, type WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { heldBack } from "@/test-held-back";
import { continueWatchingRow, pluginRow } from "@/test-home-rows";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

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
    router.service(BrowseService, {
      listHomeRows: () => ({ rows: [continueWatchingRow] }),
      listFeatured: () => ({}),
      ...browse,
    });
    router.service(WatchStateService, watchState);
  });
}

/** The Home page, with `entries` in Continue Watching. */
function openHome(...entries: MessageInitShape<typeof ContinueWatchingEntrySchema>[]) {
  openApp("/", server({ listContinueWatching: () => ({ entries }) }));
}

/** What the card `link` covers shows: its art, the preview until that loads, its Title Logo and its text. */
function looksOf(link: HTMLElement) {
  const card = link.parentElement;
  return {
    art: card?.querySelector("img:not([src^='data:'])")?.getAttribute("src") ?? undefined,
    preview: card?.querySelector("img[src^='data:']")?.getAttribute("src") ?? undefined,
    logo: card?.querySelector("img[alt]:not([alt=''])")?.getAttribute("src") ?? undefined,
    text: card?.textContent,
  };
}

/** One-pixel thumbhashes of two colours, so a titled Backdrop's preview differs from a Backdrop's. */
const backdropHash = rgbaToThumbHash(1, 1, new Uint8Array([10, 0, 0, 255]));
const titledHash = rgbaToThumbHash(1, 1, new Uint8Array([20, 0, 0, 255]));

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
      value: {
        episode: {
          id: { namespace: "tmdb", value: `${show.title}-${seasonNumber}-${episodeNumber}` },
          seasonNumber,
          episodeNumber,
          watchState,
        },
        show,
      },
    },
  } as const;
}

test("home asks for Continue Watching's first page, a shelf's worth", async () => {
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

  await waitFor(() => {
    expect(asked).not.toHaveLength(0);
  });
  // Home may refetch on mount under load (#1238), so this checks each request's page, not how many.
  const pagesAsked = asked.map(({ page }) => ({ size: page?.size, token: page?.token }));
  expect(pagesAsked).toStrictEqual(pagesAsked.map(() => ({ size: 8, token: "" })));
});

test("continue Watching doesn't ask again when home mounts over a second after its loader", async () => {
  let asked = 0;
  const transport = createRouterTransport((router) => {
    router.service(BrowseService, {
      listContinueWatching: () => {
        asked += 1;
        return {};
      },
    });
  });
  const queryClient = new QueryClient();
  await queryClient.infiniteQuery(continueWatchingQuery(transport));
  // Only Date is faked, so Suspense and the transport still run on real timers.
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(Date.now() + 2000);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <TransportProvider transport={transport}>
      <QueryClientProvider client={queryClient}>
        <Suspense>{children}</Suspense>
      </QueryClientProvider>
    </TransportProvider>
  );
  try {
    renderHook(useContinueWatching, { wrapper });
    // A refetch on mount would have started by now; wait for it to reach the Server.
    await waitFor(() => {
      expect(queryClient.isFetching()).toBe(0);
    });
  } finally {
    vi.useRealTimers();
  }

  expect(asked).toBe(1);
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

  const card = await screen.findByRole("link", { name: "Resume Alien, 1h 12m left" });
  expect(card.getAttribute("href")).toBe("/libraries/1/movies/tmdb-348/play");
  expect(looksOf(card).art).toBe("/media/artwork/tmdb/%2Falien-backdrop.jpg");
});

test("a Season 2 Episode's card shows its Show's Backdrop, not a Season's Poster", async () => {
  const dark = {
    id: { namespace: "tmdb", value: "70523" },
    title: "Dark",
    posterUrl: "/media/artwork/tmdb/%2Fdark-season-1.jpg",
    backdropUrl: "/media/artwork/tmdb/%2Fdark-backdrop.jpg",
  } as const;
  openHome(episodeEntry(dark, 2, 3, { positionMs: 1_020_000n, durationMs: 3_000_000n }));

  const card = await screen.findByRole("link", { name: "Resume Dark, S2, E3 · 33m left" });
  expect(card.parentElement?.querySelector(String.raw`.before\:w-\(--played\)`)).not.toBeNull();
  expect(card.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523/seasons/2/episodes/3/play");
  expect(looksOf(card).art).toBe("/media/artwork/tmdb/%2Fdark-backdrop.jpg");
  // Beside its bar, the card leaves out "left": the bar says it.
  expect(card.parentElement?.textContent).toContain("S2, E3 · 33m");
  expect(card.parentElement?.textContent).not.toContain("left");
});

/** Which Backdrops a title has: its plain one, its titled one, or both. */
type Backdrops = "plain" | "titled" | "both";

/** `title` with its Title Logo and the Backdrops `backdrops` names. */
function withArt(title: string, backdrops: Backdrops) {
  const plain = { backdropUrl: `/${title}-backdrop.jpg`, backdropThumbhash: backdropHash };
  const titled = { titledBackdropUrl: `/${title}-titled.jpg`, titledBackdropThumbhash: titledHash };
  const art = { plain, titled, both: { ...plain, ...titled } }[backdrops];
  return { title, titleLogoUrl: `/${title}-logo.png`, ...art };
}

/** What Alien's and Dark's Episode 2's cards show, with the Backdrops `backdrops` names. */
async function cardsWithArt(backdrops: Backdrops) {
  const movie = { id: alien, ...withArt("Alien", backdrops) };
  const dark = { id: { namespace: "tmdb", value: "70523" }, ...withArt("Dark", backdrops) };
  const alienEntry = { libraryId: 1n, movieOrEpisode: { case: "movie", value: movie } } as const;
  openHome(alienEntry, episodeEntry(dark, 1, 2));
  return [
    looksOf(await screen.findByRole("link", { name: "Play Alien" })),
    looksOf(await screen.findByRole("link", { name: "Play Dark, S1, E2" })),
  ];
}

test("a card shows its Backdrop under its Title Logo, even with a titled Backdrop", async () => {
  const preview = thumbHashToDataURL(backdropHash);
  const underLogo = [
    { art: "/Alien-backdrop.jpg", preview, logo: "/Alien-logo.png", text: "" },
    { art: "/Dark-backdrop.jpg", preview, logo: "/Dark-logo.png", text: "S1, E2" },
  ];
  await expect(cardsWithArt("both")).resolves.toStrictEqual(underLogo);
  cleanup();
  await expect(cardsWithArt("plain")).resolves.toStrictEqual(underLogo);
});

test("a card with only a titled Backdrop shows it, without its Title Logo or title", async () => {
  const preview = thumbHashToDataURL(titledHash);
  await expect(cardsWithArt("titled")).resolves.toStrictEqual([
    { art: "/Alien-titled.jpg", preview, logo: undefined, text: "" },
    { art: "/Dark-titled.jpg", preview, logo: undefined, text: "S1, E2" },
  ]);
});

test("a Show's next Episode's card plays it, saying which it is", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" } as const;
  const entry = episodeEntry(dark, 1, 4);
  openHome({
    ...entry,
    movieOrEpisode: {
      ...entry.movieOrEpisode,
      value: {
        ...entry.movieOrEpisode.value,
        episode: { ...entry.movieOrEpisode.value.episode, runtimeMinutes: 47 },
      },
    },
  });

  // Not started, it says how long it runs, with no bar.
  const card = await screen.findByRole("link", { name: "Play Dark, S1, E4 · 47m" });
  expect(card.parentElement?.querySelector(String.raw`.before\:w-\(--played\)`)).toBeNull();
  expect(card.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523/seasons/1/episodes/4/play");
});

test("a Special without a Backdrop shows its Show's title on a plain card", async () => {
  const severance = { id: { namespace: "tmdb", value: "95396" }, title: "Severance" } as const;
  openHome(episodeEntry(severance, 0, 1, { positionMs: 1n, durationMs: 60_000n }));

  const card = await screen.findByRole("link", {
    name: "Resume Severance, Specials, E1 · 1m left",
  });
  expect(looksOf(card).art).toBeUndefined();
  expect(card.parentElement?.textContent).toContain("Severance");
});

test("continue Watching hides when there's nothing", async () => {
  const heat = { libraryId: 1n, id: { namespace: "tmdb", value: "949" }, title: "Heat" };
  openApp(
    "/",
    server({
      listContinueWatching: () => ({}),
      listHomeRows: () => ({ rows: [continueWatchingRow, pluginRow("lists", "picks", "Picks")] }),
      listHomeRowTitles: () => ({ titles: [{ movieOrShow: { case: "movie", value: heat } }] }),
    }),
  );

  // The first row loads after Continue Watching, which the loader starts first.
  await screen.findByRole("link", { name: "Heat" });
  expect(screen.queryByRole("region", { name: "Continue Watching" })).toBeNull();
});

/**
 * A fake Server whose Continue Watching lists Dark's Episode 2 until it's marked watched, then its
 * Episode 3, with each Episode marked noted in `asked`.
 */
function serverMarking() {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
  const asked: { episode: string | undefined; played: boolean }[] = [];
  const transport = server(
    {
      listContinueWatching: () => ({
        entries: [episodeEntry(dark, 1, asked.length === 0 ? 2 : 3)],
      }),
    },
    {
      setPlayed: ({ target, played }) => {
        asked.push({
          episode: target.case === "episodeId" ? target.value.value : undefined,
          played,
        });
        return {};
      },
    },
  );
  return { transport, asked };
}

test("a card's menu opens its Show's page, or its Movie's", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" } as const;
  const alienEntry = {
    libraryId: 1n,
    movieOrEpisode: { case: "movie", value: { id: alien, title: "Alien" } },
  } as const;
  openHome(episodeEntry(dark, 1, 2), alienEntry);

  fireEvent.click(await screen.findByRole("button", { name: "More for Dark" }));
  const show = await screen.findByRole("menuitem", { name: "Go to Show" });
  expect(show.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523");
  fireEvent.keyDown(show, { key: "Escape" });
  fireEvent.click(await screen.findByRole("button", { name: "More for Alien" }));
  const movie = await screen.findByRole("menuitem", { name: "Go to Movie" });
  expect(movie.getAttribute("href")).toBe("/libraries/1/movies/tmdb-348");
});

test("marking an Episode watched moves its card on to the Show's next Episode", async () => {
  const { transport, asked } = serverMarking();
  openApp("/", transport);

  fireEvent.click(await screen.findByRole("button", { name: "More for Dark" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Mark as watched" }));

  await expect(screen.findByRole("link", { name: "Play Dark, S1, E3" })).resolves.toBeDefined();
  expect(asked).toStrictEqual([{ episode: "Dark-1-2", played: true }]);
});

/**
 * A fake Server whose Continue Watching lists `entry` until it's removed, with each removal asked
 * of it: whether it hid it, and which.
 */
function serverRemoving(entry: MessageInitShape<typeof ContinueWatchingEntrySchema>) {
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

test("removing a Show from recently watched takes its card away", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
  const { transport, asked } = serverRemoving(episodeEntry(dark, 1, 2));
  openApp("/", transport);

  fireEvent.click(await screen.findByRole("button", { name: "More for Dark" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from recently watched" }));

  await waitFor(() => {
    expect(screen.queryByRole("link", { name: /Dark/u })).toBeNull();
  });
  expect(asked).toStrictEqual([{ case: "showId", hidden: true }]);
});

/** Picks `item` from Dark's card's menu, which takes the card away, then the card `refuse` brings back. */
async function leavesThenComesBack(item: string, refuse: () => void) {
  fireEvent.click(await screen.findByRole("button", { name: "More for Dark" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: item }));
  await waitFor(() => {
    expect(screen.queryByRole("link", { name: /Dark/u })).toBeNull();
  });
  refuse();
  return screen.findByRole("link", { name: /Dark/u });
}

test("a card leaves as you mark it watched or remove it, and comes back if the Server refuses", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
  const marking = heldBack();
  const removing = heldBack();
  const transport = server(
    { listContinueWatching: () => ({ entries: [episodeEntry(dark, 1, 2)] }) },
    { setPlayed: marking.answer, setHiddenFromContinueWatching: removing.answer },
  );
  openApp("/", transport);

  await expect(leavesThenComesBack("Mark as watched", marking.refuse)).resolves.toBeDefined();
  await expect(
    leavesThenComesBack("Remove from recently watched", removing.refuse),
  ).resolves.toBeDefined();
});
