import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openMenu, pickFromMenu } from "@/test-menu";

afterEach(cleanup);

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

test("marking a Season played marks each of its Episodes played", async () => {
  let played = false;
  let marked: string | undefined;
  const transport = server(
    {
      getShow: () => ({ show: { title: "Severance" }, seasons: [{ number: 1 }] }),
      listEpisodes: () => ({
        episodes: [1, 2].map((episodeNumber) => ({
          id: { namespace: "tmdb", value: String(episodeNumber) },
          seasonNumber: 1,
          episodeNumber,
          title: `Episode ${episodeNumber}`,
          versions: [{ id: BigInt(episodeNumber) }],
          watchState: { played },
        })),
      }),
    },
    {
      setPlayed: (request) => {
        marked = request.target.case;
        ({ played } = request);
        return {};
      },
    },
  );
  openApp("/libraries/2/shows/tmdb-95396", transport);
  await screen.findByRole("link", { name: "Play Season 1, Episode 1" });

  await pickFromMenu("More actions", "Mark Season 1 as watched");

  // Once the Episodes come back played, the open menu offers to mark the Season unwatched.
  await openMenu("More actions");
  await expect(
    screen.findByRole("menuitem", { name: "Mark Season 1 as unwatched" }),
  ).resolves.toBeDefined();
  expect(marked).toBe("season");
});
