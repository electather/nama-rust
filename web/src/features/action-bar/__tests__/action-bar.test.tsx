import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

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

const theOffice = { namespace: "tmdb", value: "2316" };

/** Finds Office Space, The Office and one of its Episodes for "office", and else nothing. */
const searchLibraries: ServiceImpl<typeof BrowseService>["searchLibraries"] = ({ query }) =>
  query.includes("office")
    ? {
        movies: [
          {
            libraryId: 1n,
            id: { namespace: "tmdb", value: "1542" },
            title: "Office Space",
            releaseDate: "1999-02-19",
          },
        ],
        shows: [{ libraryId: 2n, id: theOffice, title: "The Office" }],
        episodes: [
          {
            libraryId: 2n,
            id: { namespace: "tmdb", value: "9001" },
            showId: theOffice,
            showTitle: "The Office",
            seasonNumber: 2,
            episodeNumber: 1,
            title: "The Dundies",
          },
        ],
      }
    : {};

const unreachable: typeof searchLibraries = () => {
  throw new ConnectError("down", Code.Internal);
};

/** Opens the action bar with ⌘K, once the app has loaded, and returns its field. */
async function openActionBar() {
  await screen.findByRole("button", { name: "Your menu" });
  fireEvent.keyDown(globalThis.window, { key: "k", metaKey: true });
  return screen.findByRole("combobox");
}

function type(field: HTMLElement, value: string) {
  fireEvent.change(field, { target: { value } });
}

/** The labels of the option rows under `heading`. */
function rowsUnder(heading: string) {
  const group = screen.getByRole("group", { name: heading });
  return within(group)
    .getAllByRole("option")
    .map((option) => option.querySelector(".truncate")?.textContent);
}

test("typing in the action bar lists the Movies, Shows and Episodes found, by kind", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  type(await openActionBar(), "office");

  await screen.findByRole("group", { name: "Movie results" });
  expect(rowsUnder("Movie results")).toStrictEqual(["Office Space"]);
  expect(rowsUnder("Show results")).toStrictEqual(["The Office"]);
  expect(rowsUnder("Episode results")).toStrictEqual(["The Dundies"]);
});

test("picking an Episode found opens its Show at that Season", async () => {
  const router = openApp("/", serverToSearch({ searchLibraries }));
  type(await openActionBar(), "office");

  fireEvent.click(await screen.findByRole("option", { name: /The Dundies/u }));

  await expect.poll(() => router.state.location.href).toBe("/libraries/2/shows/tmdb-2316?season=2");
  expect(screen.queryByRole("combobox")).toBeNull();
});

test("a title opened from the action bar is listed under Recent when it opens again", async () => {
  openApp("/", serverToSearch({ searchLibraries }));
  type(await openActionBar(), "office");
  fireEvent.click(await screen.findByRole("option", { name: /Office Space/u }));

  await openActionBar();

  expect(rowsUnder("Recent")).toStrictEqual(["Office Space"]);
});

test("search Movies narrows what's found to Movies, and Backspace widens it again", async () => {
  openApp("/", serverToSearch({ searchLibraries }));
  const field = await openActionBar();
  fireEvent.click(screen.getByRole("option", { name: /Search Movies/u }));

  type(field, "office");

  await screen.findByRole("group", { name: "Movie results" });
  expect(screen.queryByRole("group", { name: "Show results" })).toBeNull();
  type(field, "");
  fireEvent.keyDown(field, { key: "Backspace" });
  expect(screen.queryByRole("button", { name: "Clear Movie filter" })).toBeNull();
  expect(screen.getByRole("group", { name: "Jump to" })).toBeDefined();
});

test("a search that finds nothing says so", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  type(await openActionBar(), "dune");

  await expect(screen.findByText("No results for “dune”")).resolves.toBeDefined();
});

test("a search that fails offers to search again", async () => {
  let answer = unreachable;
  openApp(
    "/",
    serverToSearch({
      searchLibraries: async (...call) => {
        const answered = await answer(...call);
        return answered;
      },
    }),
  );
  type(await openActionBar(), "office");
  const retry = await screen.findByRole("option", { name: /Retry/u });
  answer = searchLibraries;

  fireEvent.click(retry);

  await expect(screen.findByRole("option", { name: /Office Space/u })).resolves.toBeDefined();
});

test("the old search page's address opens the action bar with its query typed", async () => {
  const router = openApp("/search?q=office", serverToSearch({ searchLibraries }));

  const field = await screen.findByRole<HTMLInputElement>("combobox");

  expect(field.value).toBe("office");
  await screen.findByRole("option", { name: /Office Space/u });
  fireEvent.keyDown(field, { key: "Escape" });
  await expect.poll(() => router.state.location.href).toBe("/");
});

test("the arrow keys and Enter pick a page to jump to", async () => {
  openApp("/", serverToSearch({ searchLibraries }));
  const field = await openActionBar();

  for (let row = 0; row < 4; row += 1) {
    fireEvent.keyDown(field, { key: "ArrowDown" });
  }
  fireEvent.keyDown(field, { key: "Enter" });

  await expect(screen.findByRole("heading", { name: "Settings" })).resolves.toBeDefined();
});

test("g then S jumps to Settings without opening the action bar", async () => {
  openApp("/", serverToSearch({ searchLibraries }));
  await screen.findByRole("button", { name: "Your menu" });

  fireEvent.keyDown(globalThis.window, { key: "g" });
  fireEvent.keyDown(globalThis.window, { key: "s" });

  await expect(screen.findByRole("heading", { name: "Settings" })).resolves.toBeDefined();
});

test("the keyboard shortcuts list every page's jump", async () => {
  openApp("/", serverToSearch({ searchLibraries }));
  await screen.findByRole("button", { name: "Your menu" });

  fireEvent.keyDown(globalThis.window, { key: "/", metaKey: true });

  await screen.findByRole("group", { name: "Navigate" });
  expect(rowsUnder("Navigate")).toStrictEqual([
    "Jump to Home",
    "Jump to Requests",
    "Jump to Settings",
  ]);
});

test("the top bar's search opens the action bar, and the User's menu opens Settings", async () => {
  openApp("/", serverToSearch({ searchLibraries }));

  fireEvent.click(await screen.findByRole("button", { name: "Search this Server" }));
  await expect(screen.findByRole("combobox")).resolves.toBeDefined();
  fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
  fireEvent.click(screen.getByRole("button", { name: "Your menu" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Settings" }));

  await expect(screen.findByRole("heading", { name: "Settings" })).resolves.toBeDefined();
});
