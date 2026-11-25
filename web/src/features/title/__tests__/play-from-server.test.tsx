import { type MessageInitShape, create } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type Interceptor,
  type ServiceImpl,
  createRouterTransport,
} from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { BrowseService, type VersionSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { accountHub } from "@/test-hub";
import { openMenu } from "@/test-menu";

afterEach(() => {
  cleanup();
  planned.length = 0;
  localStorage.clear();
  vi.unstubAllGlobals();
});

const alien = { namespace: "tmdb", value: "348" };

/** A Version of `gigabytes`, the largest playing unless another is chosen. */
function version(id: bigint, gigabytes: number): MessageInitShape<typeof VersionSchema> {
  return { id, fileName: "Alien.mkv", size: BigInt(gigabytes) * 1_000_000_000n };
}

/** Each plan a fake Server was asked for, as `<its Movies Library>:<Version>`. */
const planned: string[] = [];

/**
 * A fake Server whose Movies Library `libraryId`, and Shows Library one after it, browse through
 * `browse`. It signs the Hub's Account in as Ada, and plans nothing it can play.
 */
function fakeServer(libraryId: bigint, browse: Partial<ServiceImpl<typeof BrowseService>>) {
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
            libraries: [
              { id: libraryId, name: "Films", kind: LibraryKind.MOVIES },
              { id: libraryId + 1n, name: "TV", kind: LibraryKind.SHOWS },
            ],
          }),
        });
        router.service(BrowseService, { listContinueWatching: () => ({}), ...browse });
        router.service(PlaybackService, {
          getTrackPreferences: () => ({}),
          getSubtitleAppearance: () => ({}),
          getAutoplay: () => ({}),
          planPlayback: ({ versionId }) => {
            planned.push(`${libraryId}:${versionId}`);
            return {};
          },
        });
      },
      { transport: { interceptors } },
    );
}

/** A fake Server whose Movies Library `libraryId` holds Alien in `versions`. */
function holdingAlien(libraryId: bigint, versions: MessageInitShape<typeof VersionSchema>[]) {
  return fakeServer(libraryId, {
    getMovie: (request) => {
      if (request.libraryId !== libraryId) {
        throw new ConnectError("no such Movie", Code.NotFound);
      }
      return { movie: { id: alien, title: "Alien", versions } };
    },
  });
}

/** A fake Server whose Shows Library holds Severance's first Episode in `versions`. */
function holdingSeverance(libraryId: bigint, versions: MessageInitShape<typeof VersionSchema>[]) {
  const pilot = { id: { namespace: "tvdb", value: "1" }, seasonNumber: 1, episodeNumber: 1 };
  return fakeServer(libraryId - 1n, {
    getShow: () => ({ show: { title: "Severance" }, seasons: [{ number: 1 }] }),
    listEpisodes: (request) => {
      if (request.libraryId !== libraryId) {
        throw new ConnectError("no such Show", Code.NotFound);
      }
      return { episodes: [{ ...pilot, title: "Good News About Hell", versions }] };
    },
  });
}

const linked = (id: string, name: string) =>
  create(LinkedServerSchema, { id, name, addresses: [`https://${id}.example`] });

/** Opens `path` at the Hub, for an Account on Basement and Cabin, each origin's fake in `at`. */
function openAtHub(path: string, at: Record<string, ReturnType<typeof fakeServer>>) {
  vi.stubGlobal("fetch", async () => {
    await Promise.resolve();
    throw new TypeError("Failed to fetch");
  });
  const { transport } = accountHub({
    servers: [linked("basement", "Basement"), linked("cabin", "Cabin")],
  });
  openApp(path, transport, undefined, (origin, interceptors) => {
    const server = at[origin] ?? unreachable;
    return server(interceptors);
  });
}

const unreachable = () =>
  createRouterTransport(({ service }) => {
    service(ServerService, {
      getServerInfo: () => {
        throw new ConnectError("Failed to fetch", Code.Unknown);
      },
    });
  });

/** Where Play goes. */
function playHref() {
  return screen.getByRole("link", { name: /^Play/u }).getAttribute("href");
}

test("a Movie on two Servers plays by default from the one with the best Version, and from the other once picked", async () => {
  openAtHub("/libraries/1/movies/tmdb-348?server=basement", {
    "https://basement.example": holdingAlien(1n, [version(1n, 2)]),
    "https://cabin.example": holdingAlien(7n, [version(5n, 30)]),
  });

  await screen.findByRole("heading", { level: 1, name: "Alien" });
  await vi.waitFor(() => {
    expect(playHref()).toMatch(/^\/libraries\/7\/movies\/tmdb-348\/play\?.*server=cabin/u);
  });

  await openMenu("More actions");
  fireEvent.click(await screen.findByRole("menuitemradio", { name: /^Basement/u }));

  await vi.waitFor(() => {
    expect(playHref()).toMatch(
      /^\/libraries\/1\/movies\/tmdb-348\/play\?version=1&server=basement/u,
    );
  });
});

test("play plays on the Server it plays from, not the page's", async () => {
  openAtHub("/libraries/1/movies/tmdb-348?server=basement", {
    "https://basement.example": holdingAlien(1n, [version(1n, 2)]),
    "https://cabin.example": holdingAlien(7n, [version(5n, 30)]),
  });
  await vi.waitFor(() => {
    expect(playHref()).toMatch(/server=cabin/u);
  });

  fireEvent.click(screen.getByRole("link", { name: /^Play/u }));

  await vi.waitFor(() => {
    expect(planned).toStrictEqual(["7:5"]);
  });
});

test("a Server the browser can't reach isn't offered to play from", async () => {
  openAtHub("/libraries/7/movies/tmdb-348?server=cabin", {
    "https://cabin.example": holdingAlien(7n, [version(5n, 2)]),
  });

  await screen.findByRole("heading", { level: 1, name: "Alien" });
  await vi.waitFor(() => {
    expect(playHref()).toMatch(/server=cabin/u);
  });
  await openMenu("More actions");
  expect(screen.queryByRole("menuitemradio", { name: /^Basement/u })).toBeNull();
});

test("a Show on two Servers plays its next Episode from the one with the best Version", async () => {
  openAtHub("/libraries/2/shows/tvdb-371980?server=basement", {
    "https://basement.example": holdingSeverance(2n, [version(1n, 1)]),
    "https://cabin.example": holdingSeverance(8n, [version(4n, 9)]),
  });

  await screen.findByRole("heading", { level: 1, name: "Severance" });
  await vi.waitFor(() => {
    expect(playHref()).toMatch(
      /^\/libraries\/8\/shows\/tvdb-371980\/seasons\/1\/episodes\/1\/play\?server=cabin/u,
    );
  });

  await openMenu("More actions");
  fireEvent.click(await screen.findByRole("menuitemradio", { name: /^Basement/u }));

  await vi.waitFor(() => {
    expect(playHref()).toMatch(
      /^\/libraries\/2\/shows\/tvdb-371980\/seasons\/1\/episodes\/1\/play\?server=basement/u,
    );
  });
});
