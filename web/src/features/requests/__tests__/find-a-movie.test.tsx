import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FoundMovie,
  FoundMovieSchema,
  type MovieHit,
  MovieHitSchema,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { SearchService, TitleStatus } from "@/gen/nama/v1/search_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { titleParam } from "@/shared/lib/title-param";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { refused } from "@/test-refusal";
import { searchFinding, searchServer } from "@/test-search";

afterEach(cleanup);

type Found = MessageInitShape<typeof FoundMovieSchema>;

/**
 * A fake Server whose title source has `movies`, where asking for one opens a Pending Request
 * for the signed-in User (ID 1), or adds them as a Requester to the one it has.
 */
function searchableServer(movies: Found[], permissions = newUser) {
  const found = new Map(
    movies.map((init) => {
      const movie = create(FoundMovieSchema, init);
      return [titleParam(movie.id), movie];
    }),
  );
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(SearchService, {
      search: searchFinding(() => [[...found.values()].map((movie) => hitOf(movie))]),
    });
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: ({ movieId }) => {
        const movie = found.get(titleParam(movieId));
        if (!movie) {
          throw refused(Code.NotFound, "no such Movie", { reason: RefusalReason.TITLE_NOT_FOUND });
        }
        return { movie };
      },
      createRequest: ({ movieId }) => {
        const movie = found.get(titleParam(movieId)) ?? create(FoundMovieSchema);
        movie.request = create(RequestSchema, {
          state: movie.request?.state ?? RequestState.PENDING,
          requesters: [...(movie.request?.requesters ?? []), { userId: 1n, name: "Sam" }],
        });
        return { request: movie.request };
      },
    });
  });
}

/** `movie` as `Search` finds it, with its status. */
function hitOf({ id, title, year, posterUrl, libraryId, request }: FoundMovie | MovieHit) {
  const movie = { id, title, year, posterUrl, libraryId, request };
  return { title: { case: "movie" as const, value: movie }, status: statusOf(movie) };
}

function statusOf({ libraryId, request }: Pick<MovieHit, "libraryId" | "request">) {
  if (libraryId !== undefined) {
    return TitleStatus.AVAILABLE;
  }
  return request ? TitleStatus.REQUESTED : TitleStatus.NOT_ON_THIS_SERVER;
}

function tmdb(value: string) {
  return { namespace: "tmdb", value };
}

// Untyped, as each serves as a hit as well as a looked-up Movie, holding only what both have.
const notOnTheServer = { id: tmdb("438631"), title: "Dune", year: 2021 };
const inALibrary = { id: tmdb("348"), title: "Alien", year: 1979, libraryId: 4n };
const askedForByAda = {
  id: tmdb("949"),
  title: "Heat",
  year: 1995,
  request: { id: 2n, state: RequestState.APPROVED, requesters: [{ userId: 2n, name: "Ada" }] },
};

async function search(title: string) {
  fill("Find a Movie or Show", title);
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  const movies = await screen.findByRole("region", { name: `Movies for “${title}”` });
  return movies;
}

test("a Movie found that isn't on the Server shows as Pending once Requested, without a reload", async () => {
  openApp("/requests", searchableServer([notOnTheServer]));
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("Dune");
  fireEvent.click(await within(results).findByRole("button", { name: "Request Dune" }));

  await expect(within(results).findByText("Pending approval")).resolves.toBeDefined();
  expect(within(results).queryByRole("button", { name: "Request Dune" })).toBeNull();
});

test("a Movie found in a Library links to it there, with no Request button", async () => {
  openApp("/requests", searchableServer([inALibrary]));
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("Alien");

  const inYourLibrary = await within(results).findByRole("link", { name: "Alien in your Library" });
  expect(inYourLibrary.getAttribute("href")).toBe("/libraries/4/movies/tmdb-348");
  expect(within(results).queryByRole("button", { name: "Request Alien" })).toBeNull();
  const poster = within(results).getByRole("link", { name: /^Alien 1979/u });
  expect(poster.getAttribute("href")).toBe("/libraries/4/movies/tmdb-348");
});

test("a Movie someone else Requested shows its state, and Requesting it joins", async () => {
  openApp("/requests", searchableServer([askedForByAda]));
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("Heat");
  await expect(within(results).findByText("Requested")).resolves.toBeDefined();
  fireEvent.click(within(results).getByRole("button", { name: "Request Heat" }));

  await waitFor(() => {
    expect(within(results).queryByRole("button", { name: "Request Heat" })).toBeNull();
  });
  expect(within(results).getByText("Requested")).toBeDefined();
});

test("the state filter isn't shown while a search's results are", async () => {
  openApp("/requests?title=Dune", searchableServer([notOnTheServer]));

  await screen.findByText("Dune");
  expect(screen.queryByRole("button", { name: /^State:/u })).toBeNull();
});

test("a User who may not Request Movies isn't offered the search", async () => {
  openApp("/requests?title=Dune", searchableServer([notOnTheServer], [Permission.VIEW_REQUESTS]));

  await screen.findByText("No Requests yet.");
  expect(screen.queryByLabelText(/Find a/u)).toBeNull();
  expect(screen.queryByText("Dune")).toBeNull();
});

test("a Movie's page shows its Request state after Requesting it, without a reload", async () => {
  openApp(
    "/movies/tmdb-438631",
    searchableServer([{ ...notOnTheServer, overview: "Paul Atreides travels to Arrakis." }]),
  );

  await expect(screen.findByRole("heading", { name: "Dune" })).resolves.toBeDefined();
  expect(screen.getByText("Paul Atreides travels to Arrakis.")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Request Dune" }));

  await expect(screen.findByText("Pending approval")).resolves.toBeDefined();
});

test("a Movie's page shows how far its Processing Request's download has got", async () => {
  const downloading = {
    id: 2n,
    state: RequestState.PROCESSING,
    requesters: [{ userId: 1n, name: "Sam" }],
    downloads: [{ title: "Dune.2021.2160p", percent: 42 }],
  };
  openApp("/movies/tmdb-438631", searchableServer([{ ...notOnTheServer, request: downloading }]));

  const pill = await screen.findByRole("progressbar", { name: "Downloading" });
  expect(pill.getAttribute("aria-valuenow")).toBe("42");
});

test("a Movie's page for one in a Library opens it there", async () => {
  const router = openApp("/movies/tmdb-348", searchableServer([inALibrary]));

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/4/movies/tmdb-348");
  });
});

test("a Movie's page is not found for a User who may not Request Movies", async () => {
  openApp("/movies/tmdb-438631", searchableServer([notOnTheServer], [Permission.VIEW_REQUESTS]));

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});

test("a Movie's page looks up an id of any namespace", async () => {
  openApp(
    "/movies/anidb-1",
    searchableServer([{ ...notOnTheServer, id: { namespace: "anidb", value: "1" } }]),
  );

  await expect(screen.findByRole("heading", { name: "Dune" })).resolves.toBeDefined();
});

test("a Movie's page is not found when no title source has its id", async () => {
  openApp("/movies/anidb-1", searchableServer([notOnTheServer]));

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});

test("requesting a Movie a scan Matched meanwhile shows it in its Library", async () => {
  const movie = create(MovieHitSchema, notOnTheServer);
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(SearchService, { search: searchFinding(() => [[hitOf(movie)]]) });
    router.service(RequestService, {
      listRequests: () => ({}),
      createRequest: () => {
        movie.libraryId = 4n;
        throw refused(Code.FailedPrecondition, "a Library has it", {
          reason: RefusalReason.ALREADY_IN_LIBRARY,
        });
      },
    });
  });
  openApp("/requests", transport);
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("Dune");
  fireEvent.click(await within(results).findByRole("button", { name: "Request Dune" }));

  await expect(
    within(results).findByRole("link", { name: "Dune in your Library" }),
  ).resolves.toBeDefined();
  expect(within(results).queryByRole("button", { name: "Request Dune" })).toBeNull();
});

function movieHit(init: MessageInitShape<typeof MovieHitSchema>) {
  return hitOf(create(MovieHitSchema, init));
}

test("a search lists Movies Available, then Requested, then to Request, as the action bar does", async () => {
  const library = [movieHit(inALibrary)];
  const titleSource = [movieHit(notOnTheServer), movieHit(askedForByAda)];
  openApp("/requests", searchServer([library, titleSource]));
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("a");
  await within(results).findByRole("button", { name: "Request Dune" });

  const opened = within(results)
    .getAllByRole("listitem")
    .map((card) => within(card).getAllByRole("link")[0]?.getAttribute("href"));
  expect(opened).toStrictEqual([
    "/libraries/4/movies/tmdb-348",
    "/movies/tmdb-949",
    "/movies/tmdb-438631",
  ]);
});

test("a search the title source couldn't answer lists this Server's Movies and says so", async () => {
  openApp("/requests", searchServer([[movieHit(inALibrary)]], true));
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("Alien");

  await expect(
    screen.findByText("Titles to request couldn’t be searched, so only this Server’s are listed."),
  ).resolves.toBeDefined();
  expect(within(results).getByRole("link", { name: "Alien in your Library" })).toBeDefined();
});
