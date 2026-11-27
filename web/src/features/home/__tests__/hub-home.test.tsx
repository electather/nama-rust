import { create } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type Interceptor,
  type ServiceImpl,
  createRouterTransport,
} from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { openApp } from "@/test-app";
import { continueWatchingRow, pluginRow, recentlyAddedRow } from "@/test-home-rows";
import { accountHub } from "@/test-hub";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const basement = create(LinkedServerSchema, {
  id: "basement",
  name: "Basement",
  addresses: ["https://basement.example"],
});
const cabin = create(LinkedServerSchema, {
  id: "cabin",
  name: "Cabin",
  addresses: ["https://cabin.example"],
});

const alien = { namespace: "tmdb", value: "348" };
const heat = { namespace: "tmdb", value: "949" };

/** A fake Server with one Movies Library, 1, that signs the Hub's Account in as Ada. */
function fakeServer(
  browse: Partial<ServiceImpl<typeof BrowseService>>,
  watchState: Partial<ServiceImpl<typeof WatchStateService>> = {},
) {
  return (interceptors: Interceptor[]) =>
    createRouterTransport(
      (router) => {
        router.service(ServerService, { getServerInfo: () => ({ version: "0.3.0" }) });
        router.service(SignInService, {
          signInWithHubToken: () => ({ token: "session" }),
          whoAmI: () => ({ user: { id: 1n, name: "Ada" } }),
        });
        router.service(LibraryService, {
          listLibraries: () => ({
            libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }],
          }),
        });
        router.service(BrowseService, {
          listHomeRows: () => ({ rows: [continueWatchingRow] }),
          listContinueWatching: () => ({}),
          ...browse,
        });
        router.service(WatchStateService, watchState);
      },
      { transport: { interceptors } },
    );
}

const unreachable = createRouterTransport(({ service }) => {
  service(ServerService, {
    getServerInfo: () => {
      throw new ConnectError("Failed to fetch", Code.Unknown);
    },
  });
});

/** Opens the Hub's Home for an Account on Basement and Cabin, each origin's fake Server in `at`. */
function openHub(at: Record<string, ReturnType<typeof fakeServer>>) {
  // A Server that can't be reached fails the no-cors check too.
  vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
  const { transport } = accountHub({ servers: [basement, cabin] });
  openApp("/", transport, undefined, (origin, interceptors) => {
    const server = at[origin];
    return server ? server(interceptors) : unreachable;
  });
}

/** A Continue Watching entry for the Movie `id`, `title`, last watched at `updatedAt`. */
function startedMovie(id: typeof alien, title: string, updatedAt: bigint) {
  const watchState = { positionMs: 60_000n, durationMs: 6_000_000n, updatedAt };
  return {
    libraryId: 1n,
    movieOrEpisode: { case: "movie", value: { id, title, watchState } },
  } as const;
}

/** A row title for the Movie `id`, `title`, in Library 1. */
function movieTitle(id: typeof alien, title: string) {
  return { movieOrShow: { case: "movie", value: { libraryId: 1n, id, title } } } as const;
}

/**
 * The links in Home's section headed `name` now, as their names and the Server each opens on.
 * Throws till there are some, for `waitFor` to retry. It finds the section afresh each time:
 * Home's pending page, shown while the route loads for over a second, has a Continue Watching
 * section of its own, which the loaded page replaces, so one found early can be gone (#2461).
 */
function linksIn(name: string) {
  const section = within(screen.getByRole("region", { name }));
  return section.getAllByRole("link").map((link) => ({
    name: link.getAttribute("aria-label"),
    server: new URL(link.getAttribute("href") ?? "", "https://hub.example").searchParams.get(
      "server",
    ),
  }));
}

test("continue Watching at the Hub lists each title once, from the Server it was last watched on", async () => {
  openHub({
    "https://basement.example": fakeServer({
      listContinueWatching: () => ({ entries: [startedMovie(alien, "Alien", 100n)] }),
    }),
    "https://cabin.example": fakeServer({
      listContinueWatching: () => ({
        entries: [startedMovie(alien, "Alien", 300n), startedMovie(heat, "Heat", 200n)],
      }),
    }),
  });

  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Alien, 1h 39m left", server: "cabin" },
      { name: "Resume Heat, 1h 39m left", server: "cabin" },
    ]);
  });
});

test("home at the Hub merges each kind of row across the Account's Servers", async () => {
  const trending = pluginRow("discover", "trending", "Trending this week");
  openHub({
    "https://basement.example": fakeServer({
      listHomeRows: () => ({
        rows: [trending, recentlyAddedRow(1n, "Films", LibraryKind.MOVIES)],
      }),
      listHomeRowTitles: () => ({ titles: [movieTitle(alien, "Alien")] }),
      listMovies: () => ({ movies: [{ id: heat, title: "Heat" }] }),
    }),
    "https://cabin.example": fakeServer({
      listHomeRows: () => ({
        rows: [recentlyAddedRow(1n, "Movies", LibraryKind.MOVIES), trending],
      }),
      listHomeRowTitles: () => ({
        titles: [movieTitle(heat, "Heat"), movieTitle(alien, "Alien")],
      }),
      listMovies: () => ({ movies: [{ id: alien, title: "Alien" }] }),
    }),
  });

  // Each Server's titles show as it answers, so this waits for both.
  await waitFor(() => {
    expect(linksIn("Trending this week")).toStrictEqual([
      { name: "Alien", server: "basement" },
      { name: "Heat", server: "cabin" },
    ]);
  });
  await waitFor(() => {
    expect(linksIn("Recently added Movies")).toStrictEqual([
      { name: "Heat", server: "basement" },
      { name: "Alien", server: "cabin" },
    ]);
  });
});

test("an unreachable Server's rows are left out, and the others' still show", async () => {
  openHub({
    "https://cabin.example": fakeServer({
      listContinueWatching: () => ({ entries: [startedMovie(heat, "Heat", 200n)] }),
    }),
  });

  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Heat, 1h 39m left", server: "cabin" },
    ]);
  });
});

/** Continue Watching's skeleton on Home, if it shows. */
function continueWatchingSkeleton() {
  return screen.getByRole("region", { name: "Continue Watching" }).querySelector(".animate-pulse");
}

test("continue Watching at the Hub shows its skeleton till the first Server answers", async () => {
  let answerBasement: ((answer: ReturnType<typeof listing>) => void) | undefined;
  const basementAnswers = new Promise<ReturnType<typeof listing>>((resolve) => {
    answerBasement = resolve;
  });
  openHub({
    "https://basement.example": fakeServer({
      listContinueWatching: async () => {
        const answer = await basementAnswers;
        return answer;
      },
    }),
  });
  await waitFor(() => {
    expect(continueWatchingSkeleton()).not.toBeNull();
  });

  answerBasement?.(listing([startedMovie(heat, "Heat", 200n)]));

  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Heat, 1h 39m left", server: "basement" },
    ]);
  });
  expect(continueWatchingSkeleton()).toBeNull();
});

test("a Server whose Continue Watching fails is named, and the others' cards still show", async () => {
  openHub({
    "https://basement.example": fakeServer({
      listContinueWatching: () => {
        throw new ConnectError("broken", Code.Internal);
      },
    }),
    "https://cabin.example": fakeServer({
      listContinueWatching: () => listing([startedMovie(heat, "Heat", 200n)]),
    }),
  });

  const region = await screen.findByRole("region", { name: "Continue Watching" });
  await within(region).findByText("Couldn’t load Continue Watching from Basement.");
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toStrictEqual([
      { name: "Resume Heat, 1h 39m left", server: "cabin" },
    ]);
  });
});

/** `entries` as one answer of Continue Watching, `nextToken` asking for the rest. */
function listing(entries: ReturnType<typeof startedMovie>[], nextToken = "") {
  return { entries, page: { nextToken } };
}

/** The Movies `M0` to `M<count - 1>`, last watched in that order, most recent first. */
function movies(count: number) {
  return Array.from({ length: count }, (_, at) =>
    startedMovie({ namespace: "tmdb", value: `m${at}` }, `M${at}`, BigInt(1000 - at * 10)),
  );
}

/** The titles `M<from>` to `M<to - 1>`. */
function titlesFrom(from: number, to: number) {
  return Array.from({ length: to - from }, (_, at) => `M${from + at}`);
}

/** The names of the cards resuming `titles`. */
function resuming(...titles: string[]) {
  return titles.map((title) => `Resume ${title}, 1h 39m left`);
}

/**
 * A fake Server listing `list()` on Continue Watching, 20 to a page, each page's token naming its
 * last title, as the Server's keyset tokens do.
 */
function pagedServer(
  list: () => ReturnType<typeof startedMovie>[],
  watchState: Partial<ServiceImpl<typeof WatchStateService>> = {},
) {
  return fakeServer(
    {
      listContinueWatching: ({ page }) => {
        const entries = list();
        const from =
          entries.findIndex(({ movieOrEpisode }) => movieOrEpisode.value.id.value === page?.token) +
          1;
        const shown = entries.slice(from, from + 20);
        const more = from + 20 < entries.length;
        return listing(shown, more ? shown.at(-1)?.movieOrEpisode.value.id.value : "");
      },
    },
    watchState,
  );
}

test("continue Watching at the Hub pages each Server as it scrolls, merged in order, each title once", async () => {
  // Wide enough that the first pages fit across the shelf at once, so it's at its end.
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(4000);
  const basementMovies = movies(25);
  openHub({
    "https://basement.example": pagedServer(() => basementMovies),
    "https://cabin.example": fakeServer({
      listContinueWatching: () =>
        listing([
          startedMovie({ namespace: "tmdb", value: "m22" }, "M22", 905n),
          startedMovie(heat, "Heat", 805n),
        ]),
    }),
  });

  await waitFor(() => {
    expect(linksIn("Continue Watching").map(({ name }) => name)).toStrictEqual(
      resuming(
        ...titlesFrom(0, 10),
        "M22",
        ...titlesFrom(10, 20),
        "Heat",
        "M20",
        "M21",
        "M23",
        "M24",
      ),
    );
  });
});

test("a card a change moves onto a Server's next page still shows at the Hub", async () => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(4000);
  let basementMovies = movies(25);
  openHub({
    "https://basement.example": pagedServer(() => basementMovies, {
      setHiddenFromContinueWatching: () => {
        // Meanwhile Alien and Heat are watched elsewhere, which moves M19 onto the next page.
        const others = basementMovies.filter(
          ({ movieOrEpisode }) => movieOrEpisode.value.title !== "M5",
        );
        basementMovies = [
          startedMovie(alien, "Alien", 2000n),
          startedMovie(heat, "Heat", 1900n),
          ...others,
        ];
        return {};
      },
    }),
  });
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toHaveLength(25);
  });

  fireEvent.click(await screen.findByRole("button", { name: "More for M5" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from recently watched" }));

  await waitFor(() => {
    expect(linksIn("Continue Watching").map(({ name }) => name)).toStrictEqual(
      resuming("Alien", "Heat", ...titlesFrom(0, 5), ...titlesFrom(6, 25)),
    );
  });
});

/** A fake Server listing Alien on Continue Watching till it's hidden, as `hidden` records. */
function serverHidingAlien(updatedAt: bigint, hidden: Set<string>, origin: string) {
  return fakeServer(
    {
      listContinueWatching: () => ({
        entries: hidden.has(origin) ? [] : [startedMovie(alien, "Alien", updatedAt)],
      }),
    },
    {
      setHiddenFromContinueWatching: () => {
        hidden.add(origin);
        return {};
      },
    },
  );
}

test("removing a title at the Hub hides it on every Server that lists it", async () => {
  const hidden = new Set<string>();
  const basementAt = "https://basement.example";
  const cabinAt = "https://cabin.example";
  openHub({
    [basementAt]: serverHidingAlien(100n, hidden, basementAt),
    [cabinAt]: serverHidingAlien(300n, hidden, cabinAt),
  });
  await waitFor(() => {
    expect(linksIn("Continue Watching")).toHaveLength(1);
  });

  fireEvent.click(await screen.findByRole("button", { name: "More for Alien" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from recently watched" }));

  await waitFor(() => {
    expect(hidden).toStrictEqual(new Set([basementAt, cabinAt]));
  });
  await waitFor(() => {
    expect(screen.queryByRole("link", { name: /Alien/u })).toBeNull();
  });
});
