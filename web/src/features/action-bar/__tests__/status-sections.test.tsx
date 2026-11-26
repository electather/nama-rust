import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { TitleKind } from "@/gen/nama/v1/match_pb";
import { type HitSchema, SearchService, TitleStatus } from "@/gen/nama/v1/search_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { searchFinding } from "@/test-search";

afterEach(cleanup);

type Hit = MessageInitShape<typeof HitSchema>;

const { AVAILABLE, REQUESTED, NOT_ON_THIS_SERVER } = TitleStatus;

/** An id TMDB could give `title`, without the spaces a row's id can't hold. */
const idOf = (title: string) => ({ namespace: "tmdb", value: title.replaceAll(/\W/gu, "") });

function movie(title: string, status: TitleStatus): Hit {
  return { title: { case: "movie", value: { id: idOf(title), title } }, status };
}

function show(title: string, status: TitleStatus): Hit {
  return { title: { case: "show", value: { id: idOf(title), title } }, status };
}

/** Titles of the kind `of` makes, Not on this Server. */
const toRequest = (of: typeof movie, ...titles: string[]) =>
  titles.map((title) => of(title, NOT_ON_THIS_SERVER));

/** The Office and Office Space in Libraries, with an Episode of the one and an actor of both. */
const library: Hit[] = [
  {
    title: { case: "show", value: { libraryId: 2n, id: idOf("Office"), title: "The Office" } },
    status: AVAILABLE,
  },
  {
    title: { case: "movie", value: { libraryId: 1n, id: idOf("Space"), title: "Office Space" } },
    status: AVAILABLE,
  },
  {
    status: AVAILABLE,
    title: {
      case: "episode",
      value: {
        libraryId: 2n,
        id: idOf("Dundies"),
        showId: idOf("Office"),
        showTitle: "The Office",
        seasonNumber: 2,
        episodeNumber: 1,
        title: "The Dundies",
      },
    },
  },
  {
    title: { case: "person", value: { id: idOf("Carell"), name: "Steve Carell" } },
    status: AVAILABLE,
  },
];

/** Opens the action bar on a Server whose search of each kind streams its `batches`, and types. */
async function search(batches: Partial<Record<TitleKind, Hit[][]>>) {
  openApp(
    "/code",
    fakeServer({ signedIn: newUser }, {}, {}, (router) => {
      router.service(SearchService, {
        search: (request, context) =>
          searchFinding(() => batches[request.kind] ?? [])(request, context),
      });
    }),
  );
  await screen.findByRole("button", { name: "Search this Server" });
  fireEvent.keyDown(document.body, { key: "k", metaKey: true });
  const field = await screen.findByRole<HTMLInputElement>("combobox", { name: "Command menu" });
  fireEvent.change(field, { target: { value: "offic" } });
  return field;
}

/** Each section listed, by its heading, with all each of its rows says. */
function sections() {
  return screen
    .getAllByRole("group")
    .map((group) => [
      group.firstElementChild?.textContent,
      Array.from(group.querySelectorAll("[role=option]"), (row) => row.textContent),
    ]);
}

test("results list by status, Libraries' first, then six to request, Movies and Shows taking turns", async () => {
  await search({
    [TitleKind.UNSPECIFIED]: [
      library,
      [
        movie("Office Party", REQUESTED),
        ...toRequest(movie, "The Office", "The Office Mix-Up", "The Office Retrospective"),
        ...toRequest(movie, "The Official Story", "The Office Wife", "The Parole Officer"),
      ],
      [
        show("Office Ladies", REQUESTED),
        ...toRequest(show, "The Office (UK)", "The Office Superfan Episodes"),
      ],
    ],
  });

  await screen.findByRole("option", { name: /^More Movies to request/u });

  expect(sections()).toStrictEqual([
    ["In your Libraries", ["The OfficeOpen", "Office Space"]],
    ["Episodes", ["1. The DundiesThe Office · Season 2"]],
    ["People", ["Steve Carell"]],
    ["Requested", ["Office PartyRequested", "Office LadiesRequested"]],
    [
      "To request",
      [
        "The OfficeRequest",
        "The Office (UK)Request",
        "The Office Mix-UpRequest",
        "The Office Superfan EpisodesRequest",
        "The Office RetrospectiveRequest",
        "The Official StoryRequest",
        "More Movies to request",
      ],
    ],
  ]);
});

test("more Shows to request opens the search of Shows on what's typed, listing every Show", async () => {
  const shows = toRequest(show, ...["1", "2", "3", "4", "5", "6"].map((at) => `Office ${at}`));
  const movies = toRequest(movie, "Office Hours");
  const field = await search({
    [TitleKind.UNSPECIFIED]: [[], movies, shows],
    [TitleKind.SHOW]: [[], shows],
  });
  const more = await screen.findByRole("option", { name: /^More Shows to request/u });
  expect(screen.queryByRole("option", { name: /^More Movies/u })).toBeNull();

  fireEvent.click(more);
  await screen.findByRole("option", { name: /^Office 6/u });

  expect(field.value).toBe("offic");
  expect(screen.getAllByRole("option")).toHaveLength(6);
});
