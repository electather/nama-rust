import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  FoundMovieSchema,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { fakeServer, fill, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

type Found = MessageInitShape<typeof FoundMovieSchema>;

/**
 * A fake Server whose metadata source has `movies`, where asking for one opens a Pending Request
 * for the signed-in User (ID 1), or adds them as a Requester to the one it has.
 */
function searchableServer(movies: Found[], permissions = newUser) {
  const found = new Map(
    movies.map((init) => {
      const movie = create(FoundMovieSchema, init);
      return [movie.tmdbId, movie];
    }),
  );
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      searchMovies: () => ({ movies: [...found.values()] }),
      searchShows: () => ({}),
      lookUpMovie: ({ movieTmdbId }) => ({ movie: found.get(movieTmdbId) }),
      createRequest: ({ movieTmdbId }) => {
        const movie = found.get(movieTmdbId) ?? create(FoundMovieSchema);
        movie.request = create(RequestSchema, {
          state: movie.request?.state ?? RequestState.PENDING,
          requesters: [...(movie.request?.requesters ?? []), { userId: 1n, name: "Sam" }],
        });
        return { request: movie.request };
      },
    });
  });
}

const notOnTheServer: Found = { tmdbId: 438_631n, title: "Dune", year: 2021 };
const inALibrary: Found = { tmdbId: 348n, title: "Alien", year: 1979, libraryId: 4n };
const askedForByAda: Found = {
  tmdbId: 949n,
  title: "Heat",
  year: 1995,
  request: { id: 2n, state: RequestState.APPROVED, requesters: [{ userId: 2n, name: "Ada" }] },
};

function search(title: string) {
  fill("Find a Movie or Show", title);
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  return screen.findByRole("region", { name: `Movies for “${title}”` });
}

test("a Movie found that isn't on the Server shows as Pending once Requested, without a reload", async () => {
  openApp("/requests", searchableServer([notOnTheServer]));
  await screen.findByLabelText("Find a Movie or Show");

  const results = await search("Dune");
  fireEvent.click(await within(results).findByRole("button", { name: "Request Dune" }));

  expect(await within(results).findByText("Pending")).toBeDefined();
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
  expect(await within(results).findByText("Approved")).toBeDefined();
  fireEvent.click(within(results).getByRole("button", { name: "Request Heat" }));

  await waitFor(() => {
    expect(within(results).queryByRole("button", { name: "Request Heat" })).toBeNull();
  });
  expect(within(results).getByText("Approved")).toBeDefined();
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

  expect(await screen.findByRole("heading", { name: "Dune" })).toBeDefined();
  expect(screen.getByText("Paul Atreides travels to Arrakis.")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Request Dune" }));

  expect(await screen.findByText("Pending")).toBeDefined();
});

test("a Movie's page shows how far its Processing Request's download has got", async () => {
  const downloading = {
    id: 2n,
    state: RequestState.PROCESSING,
    requesters: [{ userId: 1n, name: "Sam" }],
    downloads: [{ title: "Dune.2021.2160p", percent: 42 }],
  };
  openApp("/movies/tmdb-438631", searchableServer([{ ...notOnTheServer, request: downloading }]));

  const bar = await screen.findByRole("progressbar", { name: "Downloaded" });
  expect(bar.getAttribute("aria-valuenow")).toBe("42");
  expect(screen.getByText("Dune.2021.2160p")).toBeDefined();
});

test("a Movie's page for one in a Library opens it there", async () => {
  const router = openApp("/movies/tmdb-348", searchableServer([inALibrary]));

  await waitFor(() => {
    expect(router.state.location.pathname).toBe("/libraries/4/movies/tmdb-348");
  });
});

test("a Movie's page is not found for a User who may not Request Movies", async () => {
  openApp("/movies/tmdb-438631", searchableServer([notOnTheServer], [Permission.VIEW_REQUESTS]));

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
});

test("Requesting a Movie a scan Matched meanwhile shows it in its Library", async () => {
  const movie = create(FoundMovieSchema, notOnTheServer);
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      searchMovies: () => ({ movies: [movie] }),
      searchShows: () => ({}),
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

  expect(await within(results).findByRole("link", { name: "Dune in your Library" })).toBeDefined();
  expect(within(results).queryByRole("button", { name: "Request Dune" })).toBeNull();
});
