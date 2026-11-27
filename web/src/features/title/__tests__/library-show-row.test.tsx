import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { layOutShelves } from "@/test-layout";
import { fakeServer, newUser } from "@/test-server";

beforeEach(layOutShelves);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/**
 * Dark in the TV Library with Season 1's two Episodes, watched, the first with a synopsis and a
 * Still, where the title source lists Season 2 too, without a Still, for a User with `permissions`.
 */
const backdropUrl = "/media/backdrops/dark.jpg";

function openDark(permissions: Permission[]) {
  const libraries = [{ id: 2n, name: "TV", kind: LibraryKind.SHOWS }];
  const { transport } = fakeServer(
    { signedIn: permissions, libraries },
    {
      BrowseService: {
        getShow: () => ({
          show: { id: { namespace: "tmdb", value: "70523" }, title: "Dark", backdropUrl },
          seasons: [{ number: 1 }],
        }),
        listEpisodes: () => ({
          episodes: [
            {
              id: { namespace: "tmdb", value: "1" },
              versions: [{ id: 1n }],
              seasonNumber: 1,
              episodeNumber: 1,
              title: "Secrets",
              overview: "A boy goes missing.",
              stillUrl: "/media/stills/secrets.jpg",
              watchState: { played: true },
            },
            {
              id: { namespace: "tmdb", value: "2" },
              versions: [{ id: 2n }],
              seasonNumber: 1,
              episodeNumber: 2,
              title: "Lies",
              watchState: { played: true },
            },
          ],
        }),
      },
      RequestService: {
        lookUpShow: () => ({
          show: {
            id: { namespace: "tmdb", value: "70523" },
            title: "Dark",
            seasons: [
              {
                seasonNumber: 1,
                inLibrary: true,
                episodes: [
                  { episodeNumber: 1, inLibrary: true },
                  { episodeNumber: 2, inLibrary: true },
                ],
              },
              {
                seasonNumber: 2,
                episodes: [{ episodeNumber: 1, title: "Beginnings and Endings" }],
              },
            ],
          },
        }),
      },
    },
  );
  openApp("/libraries/2/shows/tmdb-70523", transport);
}

test("a Library's Show lists the Episodes missing from the Server after its own, and Requests the next Season", async () => {
  openDark(newUser);

  const missing = await screen.findByText(
    "Season 2, Episode 1: Beginnings and Endings, Not on this Server",
  );
  expect(screen.getByRole("link", { name: /^Play Season 1, Episode 1: Secrets/u })).toBeDefined();
  expect(screen.getByRole("button", { name: "Request Season 2 of Dark" })).toBeDefined();
  // The Show's Backdrop stands in for the missing Episode's Still.
  expect(missing.closest("li")?.querySelector("img")?.getAttribute("src")).toBe(backdropUrl);
});

test("every card of a Show's row is square when one Episode has a synopsis", async () => {
  openDark(newUser);
  await screen.findByText(/^Season 2, Episode 1/u);

  const cards = within(screen.getByRole("list", { name: "Episodes" })).getAllByRole("listitem");
  expect(cards.map((card) => card.querySelector(".aspect-square") !== null)).toStrictEqual([
    true,
    true,
    true,
  ]);
});

test("a User who may not Request Shows sees only the Library's Episodes, and Plays the first once every one is watched", async () => {
  openDark([Permission.REQUEST_MOVIE]);

  await expect(
    screen.findByRole("link", { name: /^Play Season 1, Episode 1: Secrets/u }),
  ).resolves.toBeDefined();
  expect(screen.queryByText(/Not on this Server$/u)).toBeNull();
  expect(screen.queryByRole("button", { name: /^Request/u })).toBeNull();
  const play = await screen.findByRole("link", { name: "Play Season 1, Episode 1" });
  expect(play.getAttribute("href")).toBe("/libraries/2/shows/tmdb-70523/seasons/1/episodes/1/play");
});
