import type { ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

import { fakeServer, fill, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** A fake Server with a Movies Library (1) and a Shows Library (2), browsed through `browse`. */
function serverToSearch(browse: Partial<ServiceImpl<typeof BrowseService>>) {
  const libraries = [
    { id: 1n, name: "Films", kind: LibraryKind.MOVIES },
    { id: 2n, name: "TV", kind: LibraryKind.SHOWS },
  ];
  return fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, browse);
  });
}

const theOffice = { tmdb: 2316n } as const;

/** Finds The Office and one of its Episodes for any query with "office" in it, and else nothing. */
const searchLibraries: ServiceImpl<typeof BrowseService>["searchLibraries"] = ({ query }) =>
  query.includes("office")
    ? {
        shows: [
          {
            libraryId: 2n,
            id: { id: { case: "tmdb", value: theOffice.tmdb } },
            title: "The Office",
          },
        ],
        episodes: [
          {
            libraryId: 2n,
            id: { id: { case: "tmdb", value: 9001n } },
            showId: { id: { case: "tmdb", value: theOffice.tmdb } },
            showTitle: "The Office",
            seasonNumber: 2,
            episodeNumber: 1,
            title: "The Dundies: Office Party",
          },
        ],
      }
    : {};

/** Types `query` into the nav's search box, once the app shows it. */
async function search(query: string) {
  await screen.findByLabelText("Search your Libraries");
  fill("Search your Libraries", query);
}

test("typing in the nav's search box lists the Shows and Episodes it finds", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  await search("office");

  const show = await screen.findByRole("link", { name: /^The Office/u });
  expect(show.getAttribute("href")).toBe("/libraries/2/shows/tmdb-2316");
  const episode = screen.getByRole("link", { name: /The Dundies: Office Party/u });
  expect(episode.getAttribute("href")).toBe("/libraries/2/shows/tmdb-2316?season=2");
});

test("picking an Episode found opens its Show at that Season", async () => {
  openApp(
    "/",
    serverToSearch({
      searchLibraries,
      getShow: () => ({ show: { title: "The Office" }, seasons: [{ number: 1 }, { number: 2 }] }),
      listEpisodes: ({ seasonNumber }) => ({
        episodes: [
          {
            seasonNumber,
            episodeNumber: 1,
            title: `Opener of Season ${seasonNumber}`,
            versions: [{ id: 1n }],
          },
        ],
      }),
    }),
  );
  await search("office");

  fireEvent.click(await screen.findByRole("link", { name: /The Dundies: Office Party/u }));

  expect(await screen.findByText(/Opener of Season 2/u)).toBeDefined();
});

test("reloading a search's page shows what it found", async () => {
  openApp("/search?query=office", serverToSearch({ searchLibraries }));

  expect(await screen.findByRole("link", { name: /^The Office/u })).toBeDefined();
  expect(screen.getByLabelText<HTMLInputElement>("Search your Libraries").value).toBe("office");
});

test("a search that finds nothing says so", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  await search("dune");

  expect(await screen.findByText("Nothing found for “dune”.")).toBeDefined();
});

test("a number typed into the URL is searched for as words", async () => {
  const searched: string[] = [];
  openApp(
    "/search?query=1917",
    serverToSearch({
      searchLibraries: ({ query }) => {
        searched.push(query);
        return {};
      },
    }),
  );

  expect(await screen.findByText("Nothing found for “1917”.")).toBeDefined();
  expect(searched).toStrictEqual(["1917"]);
});

test("a blank query in the URL searches for nothing", async () => {
  openApp("/search?query=%20%20", serverToSearch({ searchLibraries }));

  expect(await screen.findByRole("heading", { name: "Search" })).toBeDefined();
});
