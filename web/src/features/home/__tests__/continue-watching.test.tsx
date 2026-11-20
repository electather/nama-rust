import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { type ServiceImpl, createRouterTransport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, renderHook, screen, waitFor, within } from "@testing-library/react";
import { type ReactNode, Suspense } from "react";
import { afterEach, expect, test, vi } from "vite-plus/test";

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
import { ExternalIdSchema } from "@/gen/nama/v1/match_pb";
import {
  type SetPlayedRequest,
  WatchStateService,
  type WatchStateSchema,
} from "@/gen/nama/v1/watch_state_pb";
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
 * (2), without a `watchState` when it's the Show's next Episode. The Episode's id is `episodeId`'s.
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
          id: episodeId(seasonNumber, episodeNumber),
          seasonNumber,
          episodeNumber,
          watchState,
        },
        show,
      },
    },
  } as const;
}

/** The id the fake Server gives Season `seasonNumber`'s Episode `episodeNumber`. */
function episodeId(seasonNumber: number, episodeNumber: number) {
  return { namespace: "tmdb", value: `${String(seasonNumber)}-${String(episodeNumber)}` };
}

test("home asks for Continue Watching's first page, a row's worth", async () => {
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
  // Home may refetch on mount under load (#1238), so this checks each request's page, not how many.
  const pagesAsked = asked.map(({ page }) => ({ size: page?.size, token: page?.token }));
  expect(pagesAsked).not.toHaveLength(0);
  expect(pagesAsked).toStrictEqual(pagesAsked.map(() => ({ size: 6, token: "" })));
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
  await queryClient.query(continueWatchingQuery(transport));
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
 * A fake Server whose Continue Watching lists `entry` until it's marked played, with what each
 * `SetPlayed` asked of it.
 */
function serverMarking(entry: MessageInitShape<typeof ContinueWatchingEntrySchema>) {
  let played = false;
  const asked: Pick<SetPlayedRequest, "libraryId" | "target" | "played">[] = [];
  const transport = server(
    { listContinueWatching: () => ({ entries: played ? [] : [entry] }) },
    {
      setPlayed: ({ libraryId, target, played: markedPlayed }) => {
        played = markedPlayed;
        asked.push({ libraryId, target, played: markedPlayed });
        return {};
      },
    },
  );
  return { transport, asked };
}

/** Marks the card for `title` watched from its menu, and waits for the card to leave the row. */
async function markCardWatched(title: string) {
  fireEvent.click(await screen.findByRole("button", { name: `More for ${title}` }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Mark as watched" }));
  await waitFor(() => {
    expect(screen.queryByRole("link", { name: new RegExp(title, "u") })).toBeNull();
  });
}

test("marking a Movie's card watched marks the Movie played and takes the card away", async () => {
  const { transport, asked } = serverMarking({
    libraryId: 1n,
    movieOrEpisode: {
      case: "movie",
      value: { id: alien, title: "Alien", watchState: { positionMs: 1n, durationMs: 60_000n } },
    },
  });
  openApp("/", transport);

  await markCardWatched("Alien");

  expect(asked).toStrictEqual([
    {
      libraryId: 1n,
      target: { case: "movieId", value: create(ExternalIdSchema, alien) },
      played: true,
    },
  ]);
});

test("marking an Episode's card watched marks that Episode played, not its Show", async () => {
  const dark = { id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
  const { transport, asked } = serverMarking(episodeEntry(dark, 1, 2));
  openApp("/", transport);

  await markCardWatched("Dark");

  expect(asked).toStrictEqual([
    {
      libraryId: 2n,
      target: { case: "episodeId", value: create(ExternalIdSchema, episodeId(1, 2)) },
      played: true,
    },
  ]);
});
