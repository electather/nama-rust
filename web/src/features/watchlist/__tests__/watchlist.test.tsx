import { type MessageInitShape, create } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type ConnectRouter,
  type Interceptor,
  createRouterTransport,
} from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import type { HomeRowTitleSchema } from "@/gen/nama/v1/browse_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { WatchlistService, type WatchlistTitleSchema } from "@/gen/nama/v1/watchlist_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
});

type Listed = MessageInitShape<typeof WatchlistTitleSchema>;

const alienId = { namespace: "tmdb", value: "348" };
const alienImdb = { namespace: "imdb", value: "tt0078748" };

/** Alien, added under `id`, in the Movies Library `libraryId`, or missing without one. */
function alien(id: typeof alienId, libraryId?: bigint): Listed {
  const movie = { id: alienId, title: "Alien" };
  const title: MessageInitShape<typeof HomeRowTitleSchema> =
    libraryId === undefined
      ? { movieOrShow: { case: "missingMovie", value: movie } }
      : { movieOrShow: { case: "movie", value: { ...movie, libraryId } } };
  return { id, title };
}

const dark: Listed = {
  id: { namespace: "tmdb", value: "70523" },
  title: {
    movieOrShow: {
      case: "show",
      value: { id: { namespace: "tmdb", value: "70523" }, title: "Dark", libraryId: 2n },
    },
  },
};

/**
 * A Watchlist service listing `titles`, taking off what `RemoveFromWatchlist` names, by the id it
 * was added under, and recording each removal in `removed`.
 */
function watchlistOf(titles: Listed[], removed: string[] = []) {
  let listed = titles;
  return (router: ConnectRouter) => {
    router.service(WatchlistService, {
      listWatchlist: () => ({ titles: listed }),
      removeFromWatchlist: ({ id }) => {
        removed.push(`${id?.namespace}-${id?.value}`);
        listed = listed.filter((title) => title.id?.value !== id?.value);
        return {};
      },
    });
  };
}

/** The name and address of each poster on the page, in order. */
function posters() {
  const list = screen.getByRole("list", { name: "Your Watchlist" });
  return within(list)
    .getAllByRole("link")
    .map((link) => [link.getAttribute("aria-label"), link.getAttribute("href")]);
}

test("a Server's Watchlist page shows each title, in a Library or missing", async () => {
  const titles = [dark, alien(alienImdb)];
  openApp("/watchlist", fakeServer({ signedIn: newUser }, {}, {}, watchlistOf(titles)));

  await screen.findByRole("list", { name: "Your Watchlist" });
  expect(posters()).toStrictEqual([
    ["Dark", "/libraries/2/shows/tmdb-70523"],
    ["Alien", "/movies/tmdb-348"],
  ]);
});

test("removing a title takes it off the Server's Watchlist by the id it was added under", async () => {
  const removed: string[] = [];
  const titles = [dark, alien(alienImdb, 1n)];
  openApp("/watchlist", fakeServer({ signedIn: newUser }, {}, {}, watchlistOf(titles, removed)));

  fireEvent.click(await screen.findByRole("button", { name: "Remove Alien from your Watchlist" }));

  await vi.waitFor(() => {
    expect(posters()).toStrictEqual([["Dark", "/libraries/2/shows/tmdb-70523"]]);
  });
  expect(removed).toStrictEqual(["imdb-tt0078748"]);
});

test("an empty Watchlist says so", async () => {
  openApp("/watchlist", fakeServer({ signedIn: newUser }, {}, {}, watchlistOf([])));

  await expect(screen.findByText("Nothing on your Watchlist yet")).resolves.toBeDefined();
});

/** A fake Server at the Hub, signing the Account in, whose Watchlist is as `watchlistOf` serves. */
function serverAtHub(...watchlist: Parameters<typeof watchlistOf>) {
  return (interceptors: Interceptor[]) =>
    createRouterTransport(
      (router) => {
        router.service(ServerService, { getServerInfo: () => ({ version: "0.3.0" }) });
        router.service(SignInService, {
          signInWithHubToken: () => ({ token: "session" }),
          whoAmI: () => ({ user: { id: 1n, name: "Ada" } }),
        });
        watchlistOf(...watchlist)(router);
      },
      { transport: { interceptors } },
    );
}

const unreachable = () =>
  createRouterTransport(({ service }) => {
    service(ServerService, {
      getServerInfo: () => {
        throw new ConnectError("Failed to fetch", Code.Unknown);
      },
    });
  });

const linked = (id: string, name: string) =>
  create(LinkedServerSchema, { id, name, addresses: [`https://${id}.example`] });

/** Opens the Watchlist at the Hub, for an Account on Basement and Cabin, each origin's fake in `at`. */
function openAtHub(at: Record<string, ReturnType<typeof serverAtHub>>) {
  vi.stubGlobal("fetch", async () => {
    await Promise.resolve();
    throw new TypeError("Failed to fetch");
  });
  const { transport } = accountHub({
    servers: [linked("basement", "Basement"), linked("cabin", "Cabin")],
  });
  openApp("/watchlist", transport, undefined, (origin, interceptors) => {
    const server = at[origin] ?? unreachable;
    return server(interceptors);
  });
}

test("at the Hub, a title on two Servers' Watchlists shows once, from the one with it in a Library", async () => {
  openAtHub({
    "https://basement.example": serverAtHub([alien(alienId)]),
    "https://cabin.example": serverAtHub([alien(alienImdb, 7n), dark]),
  });

  await vi.waitFor(() => {
    expect(posters()).toStrictEqual([
      ["Alien", "/libraries/7/movies/tmdb-348?server=cabin"],
      ["Dark", "/libraries/2/shows/tmdb-70523?server=cabin"],
    ]);
  });
});

test("at the Hub, removing a title takes it off every Server's Watchlist", async () => {
  const basement: string[] = [];
  const cabin: string[] = [];
  openAtHub({
    "https://basement.example": serverAtHub([alien(alienId)], basement),
    "https://cabin.example": serverAtHub([alien(alienImdb, 7n), dark], cabin),
  });
  await vi.waitFor(() => {
    expect(posters()).toHaveLength(2);
  });

  fireEvent.click(screen.getByRole("button", { name: "Remove Alien from your Watchlist" }));

  await vi.waitFor(() => {
    expect(posters()).toStrictEqual([["Dark", "/libraries/2/shows/tmdb-70523?server=cabin"]]);
  });
  expect([basement, cabin]).toStrictEqual([["tmdb-348"], ["imdb-tt0078748"]]);
});

test("at the Hub, a Server the browser can't reach leaves the others' titles showing", async () => {
  openAtHub({ "https://cabin.example": serverAtHub([dark]) });

  await vi.waitFor(() => {
    expect(posters()).toStrictEqual([["Dark", "/libraries/2/shows/tmdb-70523?server=cabin"]]);
  });
});
