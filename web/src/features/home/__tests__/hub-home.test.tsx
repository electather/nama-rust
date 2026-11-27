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
const arrival = { namespace: "tmdb", value: "329865" };

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
    server: serverOf(link),
  }));
}

/** The Server `link` opens on at the Hub. */
function serverOf(link: HTMLElement) {
  return new URL(link.getAttribute("href") ?? "", "https://hub.example").searchParams.get("server");
}

/** A Featured Movie `title`, `id`, in Library 1. */
function featuredMovie(id: typeof alien, title: string) {
  return { libraryId: 1n, movieOrShow: { case: "movie", value: { id, title } } } as const;
}

/** Featured's slides now, as their titles and the Server each plays on. Throws till there's one. */
function featuredSlides() {
  const featured = within(screen.getByRole("region", { name: "Featured" }));
  return featured.getAllByRole("listitem").map((slide) => ({
    name: within(slide).getByRole("heading").textContent,
    server: serverOf(within(slide).getByRole("link")),
  }));
}

test("featured at the Hub takes each Server's in turn, each title once, opening on its Server", async () => {
  openHub({
    "https://basement.example": fakeServer({
      listFeatured: () => ({
        titles: [featuredMovie(alien, "Alien"), featuredMovie(heat, "Heat")],
      }),
    }),
    "https://cabin.example": fakeServer({
      listFeatured: () => ({
        titles: [featuredMovie(arrival, "Arrival"), featuredMovie(alien, "Alien")],
      }),
    }),
  });

  await waitFor(() => {
    expect(featuredSlides()).toStrictEqual([
      { name: "Alien", server: "basement" },
      { name: "Arrival", server: "cabin" },
      { name: "Heat", server: "basement" },
    ]);
  });
});

test("a failing Server's Featured is left out, and the others' still show", async () => {
  openHub({
    "https://basement.example": fakeServer({
      listFeatured: () => {
        throw new ConnectError("database is locked", Code.Unavailable);
      },
    }),
    "https://cabin.example": fakeServer({
      listFeatured: () => ({ titles: [featuredMovie(heat, "Heat")] }),
    }),
  });

  await waitFor(() => {
    expect(featuredSlides()).toStrictEqual([{ name: "Heat", server: "cabin" }]);
  });
});

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
