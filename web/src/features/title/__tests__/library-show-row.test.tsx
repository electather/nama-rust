import { cleanup, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { layOutShelves } from "@/test-layout";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/**
 * Dark in the TV Library with Season 1's one Episode, watched, where the title source lists
 * Season 2 too, for a User with `permissions`.
 */
function openDark(permissions: Permission[]) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  const transport = fakeServer({ signedIn: permissions, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      getShow: () => ({
        show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark" },
        seasons: [{ number: 1 }],
      }),
      listContinueWatching: () => ({}),
      listEpisodes: () => ({
        episodes: [
          {
            id: { namespace: "tmdb", value: "1" },
            versions: [{ id: 1n }],
            seasonNumber: 1,
            episodeNumber: 1,
            title: "Secrets",
            watchState: { played: true },
          },
        ],
      }),
    });
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpShow: () => ({
        show: {
          id: { namespace: "tmdb", value: "70523" },
          title: "Dark",
          seasons: [
            { seasonNumber: 1, inLibrary: true, episodes: [{ episodeNumber: 1, inLibrary: true }] },
            { seasonNumber: 2, episodes: [{ episodeNumber: 1, title: "Beginnings and Endings" }] },
          ],
        },
      }),
    });
  });
  openApp("/libraries/2/shows/tmdb-70523", transport);
}

test("a Library's Show lists the Episodes missing from the Server after its own, and Requests the next Season", async () => {
  openDark(newUser);

  await expect(
    screen.findByText("Season 2, Episode 1: Beginnings and Endings, Not on this Server"),
  ).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: /^Play Season 1, Episode 1: Secrets/u })).toBeDefined();
  expect(screen.getByRole("button", { name: "Request Season 2 of Dark" })).toBeDefined();
});

test("a User who may not Request Shows sees only the Library's Episodes", async () => {
  openDark([Permission.REQUEST_MOVIE]);

  await expect(
    screen.findByRole("link", { name: /^Play Season 1, Episode 1: Secrets/u }),
  ).resolves.toBeDefined();
  expect(screen.queryByText(/Not on this Server$/u)).toBeNull();
  expect(screen.queryByRole("button", { name: /^Request/u })).toBeNull();
});
