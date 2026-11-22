import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { rgbaToThumbHash, thumbHashToDataURL } from "thumbhash";
import { afterEach, expect, test } from "vite-plus/test";

import { TitleKind } from "@/gen/nama/v1/match_pb";
import { type MovieHitSchema, RequestState, type ShowHitSchema } from "@/gen/nama/v1/request_pb";
import {
  type EpisodeHitSchema,
  type SearchResponseSchema,
  SearchService,
  TitleStatus,
} from "@/gen/nama/v1/search_pb";
import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { modKey } from "@/shared/lib/mod-key";
import { admin, fakeServer, newUser, openApp } from "@/test-app";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

type Search = ServiceImpl<typeof SearchService>["search"];

const tmdb = (value: string) => ({ namespace: "tmdb", value });

const officeSpace = {
  case: "movie",
  value: { libraryId: 1n, id: tmdb("1542"), title: "Office Space", year: 1999 },
} as const;
const theOffice = {
  case: "show",
  value: { libraryId: 2n, id: tmdb("2316"), title: "The Office" },
} as const;
const theDundies = {
  case: "episode",
  value: {
    libraryId: 2n,
    id: tmdb("9001"),
    showId: tmdb("2316"),
    showTitle: "The Office",
    seasonNumber: 2,
    episodeNumber: 1,
    title: "The Dundies",
  },
} as const;

/** The titles a search for "office" of each kind finds: a Movie, a Show and one of its Episodes. */
const office: Record<
  TitleKind,
  readonly (typeof officeSpace | typeof theOffice | typeof theDundies)[]
> = {
  [TitleKind.UNSPECIFIED]: [officeSpace, theOffice, theDundies],
  [TitleKind.MOVIE]: [officeSpace],
  [TitleKind.SHOW]: [theOffice, theDundies],
};

/** Finds the titles of the kind asked for if the query has "office", in one batch, then Done. */
const findsOffice: Search = ({ query, kind }) => {
  const titles = (query.includes("office") ? office[kind] : undefined) ?? [];
  return new ReadableStream<MessageInitShape<typeof SearchResponseSchema>>({
    start(controller) {
      controller.enqueue({
        part: { case: "hits", value: { hits: titles.map((title) => ({ title })) } },
      });
      controller.enqueue({ part: { case: "done", value: {} } });
      controller.close();
    },
  });
};

type Part = MessageInitShape<typeof SearchResponseSchema>;

/** Streams `parts` for any search. */
function streams(...parts: Part[]): Search {
  return () =>
    new ReadableStream<Part>({
      start(controller) {
        for (const part of parts) {
          controller.enqueue(part);
        }
        controller.close();
      },
    });
}

/** A batch of Movies, each with its status. */
function movies(...hits: [MessageInitShape<typeof MovieHitSchema>, TitleStatus][]): Part {
  const batch = hits.map(([value, status]) => ({
    title: { case: "movie" as const, value },
    status,
  }));
  return { part: { case: "hits", value: { hits: batch } } };
}

/** A batch of Shows, each with its status. */
function shows(...hits: [MessageInitShape<typeof ShowHitSchema>, TitleStatus][]): Part {
  const batch = hits.map(([value, status]) => ({
    title: { case: "show" as const, value },
    status,
  }));
  return { part: { case: "hits", value: { hits: batch } } };
}

/** A search the Server fails. */
const down: Search = () => {
  throw new ConnectError("down", Code.Internal);
};

/** A fake Server that answers a search with `search`, for a User with `permissions`. */
function serverToSearch(search: Search, permissions: Permission[] = newUser) {
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(SearchService, { search });
  });
}

/** Opens the action bar with its keys, once the page is up, and gives its field. */
async function openActionBar() {
  await screen.findByRole("button", { name: "Search this Server" });
  fireEvent.keyDown(document.body, { key: "k", metaKey: true });
  const field = await screen.findByRole("combobox", { name: "Command menu" });
  return field;
}

function type(field: HTMLElement, value: string) {
  fireEvent.change(field, { target: { value } });
}

/** The rows listed, top to bottom, each by all it says. */
function rows() {
  return screen.getAllByRole("option").map((row) => row.textContent);
}

test("⌘K opens the action bar on the pages a User may jump to, and closes it", async () => {
  openApp("/code", serverToSearch(findsOffice));

  await openActionBar();

  expect(rows()).toStrictEqual([
    "Search ShowsType to search the Shows in your LibrariesOpen",
    "Search MoviesType to search the Movies in your Libraries",
    "HomeContinue Watching and your Libraries",
    "RequestsAsk for a Movie or Show, and follow it",
    "SettingsYour password, Devices and playback",
    `Keyboard shortcuts${modKey}/View every active hotkey`,
  ]);
  fireEvent.keyDown(document.body, { key: "k", metaKey: true });
  await waitFor(() => {
    expect(screen.queryByRole("combobox", { name: "Command menu" })).toBeNull();
  });
});

test("an Admin has no Admin page to jump to, as none is left", async () => {
  openApp("/code", serverToSearch(findsOffice, admin));

  await openActionBar();

  expect(screen.queryByRole("option", { name: /^Admin/u })).toBeNull();
});

test("typing lists the titles found, Enter opens the first, and it's a Recent next time", async () => {
  const router = openApp("/code", serverToSearch(findsOffice));
  const field = await openActionBar();

  type(field, "office");
  await screen.findByRole("option", { name: /^Office Space/u });

  expect(rows()).toStrictEqual([
    "Office Space1999Open",
    "The Office",
    "1. The DundiesThe Office · Season 2",
  ]);
  fireEvent.keyDown(field, { key: "Enter" });
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/1/movies/tmdb-1542");
  });

  await openActionBar();
  expect(rows()).toContain("Office SpaceMovie1999");
});

test("an Episode found opens its Show at its Season", async () => {
  const router = openApp("/code", serverToSearch(findsOffice));
  const field = await openActionBar();

  type(field, "office");
  fireEvent.click(await screen.findByRole("option", { name: /The Dundies/u }));

  await waitFor(() => {
    expect(router.state.location.href).toBe("/libraries/2/shows/tmdb-2316?season=2");
  });
});

type EpisodeArt = Pick<
  MessageInitShape<typeof EpisodeHitSchema>,
  "stillUrl" | "stillThumbhash" | "showPosterUrl"
>;

/** The Dundies, called `title`, with `art`. */
function episode(title: string, art: EpisodeArt) {
  return { title: { case: "episode" as const, value: { ...theDundies.value, title, ...art } } };
}

/** The sources of the images the row `name` names leads with, back to front. */
function artOf(name: RegExp) {
  return Array.from(screen.getByRole("option", { name }).querySelectorAll("img"), (img) =>
    img.getAttribute("src"),
  );
}

test("an Episode found leads with its Still, else its Show's Poster, else its icon, and Recent keeps it", async () => {
  const still = {
    stillUrl: "/still.jpg",
    stillThumbhash: rgbaToThumbHash(1, 1, new Uint8Array(4)),
  };
  const poster = { showPosterUrl: "/office.jpg" };
  const batch = [
    episode("Still", { ...still, ...poster }),
    episode("Poster", poster),
    episode("Neither", {}),
  ];
  openApp("/code", serverToSearch(streams({ part: { case: "hits", value: { hits: batch } } })));
  const field = await openActionBar();

  type(field, "office");
  await screen.findByRole("option", { name: /Still/u });

  expect(artOf(/Still/u)).toStrictEqual([thumbHashToDataURL(still.stillThumbhash), "/still.jpg"]);
  expect(artOf(/Poster/u)).toStrictEqual(["/office.jpg"]);
  expect(artOf(/Neither/u)).toStrictEqual([]);
  fireEvent.click(screen.getByRole("option", { name: /Still/u }));
  cleanup();
  openApp("/code", serverToSearch(findsOffice));
  await openActionBar();
  // Recent comes back from storage as JSON, which a thumbhash's bytes don't survive.
  expect(artOf(/Still/u)).toStrictEqual(["/still.jpg"]);
});

test("a search of one kind asks the Server for that kind, and Escape steps back out of it", async () => {
  const kinds: TitleKind[] = [];
  const search: Search = (request, context) => {
    kinds.push(request.kind);
    return findsOffice(request, context);
  };
  openApp("/code", serverToSearch(search));
  const field = await openActionBar();

  fireEvent.click(screen.getByRole("option", { name: /^Search Shows/u }));
  type(field, "office");
  await screen.findByRole("option", { name: /^The Office/u });

  expect(rows()).toStrictEqual(["The OfficeOpen", "1. The DundiesThe Office · Season 2"]);
  expect(kinds).toStrictEqual([TitleKind.SHOW]);
  fireEvent.keyDown(field, { key: "Escape" });
  expect(screen.getByRole("option", { name: /^Search Movies/u })).toBeDefined();
  expect(screen.queryByRole("button", { name: "Clear filter" })).toBeNull();
});

test("a search that finds nothing says so", async () => {
  openApp("/code", serverToSearch(findsOffice));
  const field = await openActionBar();

  type(field, "dune");

  await expect(screen.findByText("No results for “dune”")).resolves.toBeDefined();
});

test("a search that fails offers Retry, which searches again", async () => {
  let answer = down;
  const search: Search = (request, context) => answer(request, context);
  openApp("/code", serverToSearch(search));
  const field = await openActionBar();

  type(field, "office");
  const retry = await screen.findByRole("option", { name: /^Retry/u });
  answer = findsOffice;
  fireEvent.click(retry);

  await expect(screen.findByRole("option", { name: /^The Office/u })).resolves.toBeDefined();
});

test("g then a page's key goes to that page", async () => {
  const router = openApp("/code", serverToSearch(findsOffice));
  await screen.findByRole("button", { name: "Search this Server" });

  fireEvent.keyDown(document.body, { key: "g" });
  fireEvent.keyDown(document.body, { key: "r" });

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/requests");
  });
  // Settings opens over the page it's asked for on.
  fireEvent.keyDown(document.body, { key: "g" });
  fireEvent.keyDown(document.body, { key: "s" });
  await screen.findByRole("dialog", { name: "Settings" });
  expect(router.state.location.pathname).toBe("/requests");
});

test("a link to the old search page opens the action bar on what it searched for", async () => {
  const router = openApp("/search?q=office", serverToSearch(findsOffice));

  const field = await screen.findByRole<HTMLInputElement>("combobox", { name: "Command menu" });

  expect(field.value).toBe("office");
  await expect(screen.findByRole("option", { name: /^The Office/u })).resolves.toBeDefined();
  await waitFor(() => {
    expect(router.state.location.href).toBe("/");
  });
});

test("an older search link's number is searched for as words", async () => {
  openApp("/search?query=1917", serverToSearch(findsOffice));

  const field = await screen.findByRole<HTMLInputElement>("combobox", { name: "Command menu" });

  expect(field.value).toBe("1917");
});

test("movies to request list below the Library's, by status, and open their request page", async () => {
  const toRequest = { id: tmdb("1"), title: "Office Hours" };
  const requested = {
    id: tmdb("2"),
    title: "Office Party",
    request: { state: RequestState.APPROVED },
  };
  const elsewhere = { libraryId: 1n, id: tmdb("3"), title: "Bureau" };
  const search = streams(
    movies([officeSpace.value, TitleStatus.AVAILABLE]),
    movies(
      [toRequest, TitleStatus.NOT_ON_THIS_SERVER],
      [requested, TitleStatus.REQUESTED],
      [elsewhere, TitleStatus.AVAILABLE],
    ),
    { part: { case: "done", value: {} } },
  );
  const router = openApp("/code", serverToSearch(search));
  const field = await openActionBar();

  type(field, "office");
  const officeHours = await screen.findByRole("option", { name: /^Office Hours/u });

  expect(rows()).toStrictEqual([
    "Office Space1999Open",
    "Bureau",
    "Office PartyRequested",
    "Office HoursRequest",
  ]);
  fireEvent.click(officeHours);
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/movies/tmdb-1");
  });
});

test("shows to request list by status, with their Request or a Requested tag, and open their request page", async () => {
  const toRequest = { id: tmdb("2996"), title: "The Office (UK)" };
  const followed = { id: tmdb("1"), title: "Office Ladies" };
  const asked = { id: tmdb("2"), title: "Office Girls", request: { state: RequestState.PENDING } };
  const search = streams(
    shows([theOffice.value, TitleStatus.AVAILABLE]),
    shows(
      [toRequest, TitleStatus.NOT_ON_THIS_SERVER],
      [followed, TitleStatus.REQUESTED],
      [asked, TitleStatus.REQUESTED],
    ),
    { part: { case: "done", value: {} } },
  );
  const router = openApp("/code", serverToSearch(search));
  const field = await openActionBar();

  type(field, "office");
  const uk = await screen.findByRole("option", { name: /^The Office \(UK\)/u });

  expect(rows()).toStrictEqual([
    "The OfficeOpen",
    "Office LadiesRequested",
    "Office GirlsPending approval",
    "The Office (UK)Request",
  ]);
  fireEvent.click(uk);
  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/shows/tmdb-2996");
  });
});

test("a search whose title source failed lists the Library's titles with a quiet note", async () => {
  const search = streams(movies([officeSpace.value, TitleStatus.AVAILABLE]), {
    part: { case: "done", value: { titleSourceUnavailable: true } },
  });
  openApp("/code", serverToSearch(search));
  const field = await openActionBar();

  type(field, "office");

  await expect(
    screen.findByText("Titles to request couldn’t be searched, so only this Server’s are listed."),
  ).resolves.toBeDefined();
  expect(rows()).toStrictEqual(["Office Space1999Open"]);
});
