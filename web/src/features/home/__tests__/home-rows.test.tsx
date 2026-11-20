import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService, type ListHomeRowTitlesRequest } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { RequestState } from "@/gen/nama/v1/request_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

const heat = { libraryId: 1n, id: { namespace: "tmdb", value: "949" }, title: "Heat" };
const dark = { libraryId: 2n, id: { namespace: "tmdb", value: "70523" }, title: "Dark" };
const dune = {
  id: { namespace: "tmdb", value: "438631" },
  title: "Dune",
  year: 2021,
  posterUrl: "/media/artwork/tmdb/%2Fdune.jpg",
  request: { id: 7n, state: RequestState.PENDING, requesters: [{ userId: 9n, name: "Ada" }] },
};
const severance = { id: { namespace: "tmdb", value: "95396" }, title: "Severance", year: 2022 };

/** Home on a Server whose Plugins give the rows `picks`, `empty` and `shows`, in that order. */
function openHome(asked: ListHomeRowTitlesRequest[] = []) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  const transport = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, {
      listContinueWatching: () => ({}),
      listHomeRows: () => ({
        rows: [
          { pluginId: "lists", key: "picks", heading: "Picks" },
          { pluginId: "lists", key: "empty", heading: "Nothing here" },
          { pluginId: "trakt", key: "shows", heading: "Trending Shows" },
        ],
      }),
      listHomeRowTitles: (request) => {
        asked.push(request);
        if (request.key === "picks") {
          return {
            titles: [
              { movieOrShow: { case: "movie", value: heat } },
              { movieOrShow: { case: "missingMovie", value: dune } },
              { movieOrShow: { case: "missingShow", value: severance } },
            ],
          };
        }
        if (request.key === "shows") {
          return { titles: [{ movieOrShow: { case: "show", value: dark } }] };
        }
        return {};
      },
    });
  });
  openApp("/", transport);
}

test("home shows each Plugin's row after the Libraries, in order, with its titles' posters", async () => {
  openHome();

  const shows = await screen.findByRole("region", { name: "Trending Shows" });
  const picks = screen.getByRole("region", { name: "Picks" });
  const headings = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);

  expect(headings).toStrictEqual(["Libraries", "Picks", "Trending Shows"]);
  expect(within(picks).getByRole("link", { name: /Heat/u }).getAttribute("href")).toBe(
    "/libraries/1/movies/tmdb-949",
  );
  expect(within(shows).getByRole("link", { name: /Dark/u }).getAttribute("href")).toBe(
    "/libraries/2/shows/tmdb-70523",
  );
});

test("home leaves out a row with no titles, and asks each row for one page", async () => {
  const asked: ListHomeRowTitlesRequest[] = [];
  openHome(asked);

  await screen.findByRole("region", { name: "Trending Shows" });

  expect(screen.queryByRole("region", { name: "Nothing here" })).toBeNull();
  expect(asked.map((request) => [request.pluginId, request.key, request.page?.size])).toStrictEqual(
    [
      ["lists", "picks", 10],
      ["lists", "empty", 10],
      ["trakt", "shows", 10],
    ],
  );
});

test("a missing title in a row opens its page to Request it, and shows its Request's state", async () => {
  openHome();

  const picks = await screen.findByRole("region", { name: "Picks" });

  const dunePoster = within(picks).getByRole("link", { name: /Dune/u });
  expect(dunePoster.getAttribute("href")).toBe("/movies/tmdb-438631");
  expect(dunePoster.querySelector("img")?.getAttribute("src")).toBe(dune.posterUrl);
  expect(within(picks).getByText("Pending")).toBeDefined();
  expect(
    within(picks)
      .getByRole("link", { name: /Severance/u })
      .getAttribute("href"),
  ).toBe("/shows/tmdb-95396");
});
