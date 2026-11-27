import { create } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type Interceptor,
  type ServiceImpl,
  createRouterTransport,
} from "@connectrpc/connect";
import { screen, within } from "@testing-library/react";
import { vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { openApp } from "@/test-app";
import { continueWatchingRow } from "@/test-home-rows";
import { accountHub } from "@/test-hub";
import { atOrigin } from "@/test-server-at";

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

/** The Movie Alien's id. */
export const alien = { namespace: "tmdb", value: "348" };
/** The Movie Heat's id. */
export const heat = { namespace: "tmdb", value: "949" };
/** The Movie Arrival's id. */
export const arrival = { namespace: "tmdb", value: "329865" };

/** A fake Server with one Movies Library, 1, that signs the Hub's Account in as Ada. */
export function fakeLinkedServer(
  browse: Partial<ServiceImpl<typeof BrowseService>>,
  watchState: Partial<ServiceImpl<typeof WatchStateService>> = {},
) {
  return (interceptors: Interceptor[]) =>
    createRouterTransport(
      (router) => {
        router.service(ServerService, {
          getServerInfo: () => ({ version: "0.3.0", imageWidths: [320] }),
        });
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
export function openHub(at: Record<string, ReturnType<typeof fakeLinkedServer>>) {
  // A Server that can't be reached fails the no-cors check too.
  vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
  const { transport } = accountHub({ servers: [basement, cabin] });
  openApp("/", transport, undefined, (origin, interceptors) => {
    const server = at[origin];
    return server ? atOrigin(server(interceptors), origin) : unreachable;
  });
}

/** A Continue Watching entry for the Movie `id`, `title`, last watched at `updatedAt`. */
export function startedMovie(id: typeof alien, title: string, updatedAt: bigint) {
  const watchState = { positionMs: 60_000n, durationMs: 6_000_000n, updatedAt };
  return {
    libraryId: 1n,
    movieOrEpisode: { case: "movie", value: { id, title, watchState } },
  } as const;
}

/**
 * The links in Home's section headed `name` now, as their names and the Server each opens on.
 * Throws till there are some, for `waitFor` to retry. It finds the section afresh each time:
 * Home's pending page, shown while the route loads for over a second, has a Continue Watching
 * section of its own, which the loaded page replaces, so one found early can be gone (#2461).
 */
export function linksIn(name: string) {
  const section = within(screen.getByRole("region", { name }));
  return section.getAllByRole("link").map((link) => ({
    name: link.getAttribute("aria-label"),
    server: serverOf(link),
  }));
}

/** The Server `link` opens on at the Hub. */
export function serverOf(link: HTMLElement) {
  return new URL(link.getAttribute("href") ?? "", "https://hub.example").searchParams.get("server");
}
