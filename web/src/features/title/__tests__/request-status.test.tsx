import { type MessageInitShape, create, fromJson } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  DownloadStatus,
  FoundMovieSchema,
  FoundShowSchema,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

type Request = MessageInitShape<typeof RequestSchema>;

/** A Request of Dune's in `state`, by the signed-in User (ID 1), with `more` as given. */
function yours(state: RequestState, more: Request = {}): Request {
  return { id: 5n, state, requesters: [{ userId: 1n, name: "Sam" }], ...more };
}

/**
 * A fake Server for Dune's page, whose Request is `request`, signed in with `permissions`. Each
 * Retry puts the Request's id in `retried`.
 */
function server(request: Request, permissions: Permission[] = newUser, retried: bigint[] = []) {
  const movie = create(FoundMovieSchema, {
    id: { namespace: "tmdb", value: "438631" },
    title: "Dune",
    year: 2021,
    score: 7.8,
    originalLanguage: "en",
    cast: [{ name: "Timothée Chalamet", part: "Paul Atreides" }],
    crew: [{ name: "Denis Villeneuve", part: "Director" }],
    genres: ["Science Fiction", "Adventure"],
    contentRating: "PG-13",
    trailers: [{ name: "Official Trailer", url: "https://www.youtube.com/watch?v=n9xhJrPXop4" }],
    request,
  });
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: () => ({ movie }),
      listMovieQualities: () => ({}),
      retryRequest: ({ id }) => {
        retried.push(id);
        return {};
      },
    });
  });
}

/** Dune's Request in a state this app doesn't know, as only a Server newer than it sends. */
const fromANewerServer = fromJson(RequestSchema, {
  id: "5",
  state: 99,
  requesters: [{ userId: "1", name: "Sam" }],
});

test.each([
  { request: yours(RequestState.PENDING), label: "Pending approval", requestable: false },
  { request: yours(RequestState.APPROVED), label: "Requested", requestable: false },
  { request: yours(RequestState.PROCESSING), label: "Processing", requestable: false },
  {
    request: yours(RequestState.PARTIALLY_AVAILABLE),
    label: "Partially Available",
    requestable: false,
  },
  { request: yours(RequestState.AVAILABLE), label: "Available", requestable: false },
  { request: yours(RequestState.DECLINED), label: "Declined", requestable: true },
  { request: fromANewerServer, label: "Unknown", requestable: false },
])("a Request shows as $label", async ({ request, label, requestable }) => {
  openApp("/movies/tmdb-438631", server(request));

  await expect(screen.findByText(label)).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Request Dune" }) !== null).toBe(requestable);
});

test("a Request downloading fills its pill to the percent, and says how long is left", async () => {
  const downloads = [
    {
      title: "Dune.2021.2160p",
      percent: 40,
      etaSeconds: 5400n,
      status: DownloadStatus.DOWNLOADING,
    },
  ];
  openApp("/movies/tmdb-438631", server(yours(RequestState.PROCESSING, { downloads })));

  const pill = await screen.findByRole("progressbar", { name: "Downloading · 1 h 30 min left" });

  expect(pill.getAttribute("aria-valuenow")).toBe("40");
});

test("a download with no time left says only that it's downloading", async () => {
  const downloads = [{ title: "Dune.2021.2160p", percent: 99, etaSeconds: 0n }];
  openApp("/movies/tmdb-438631", server(yours(RequestState.PROCESSING, { downloads })));

  await expect(screen.findByRole("progressbar", { name: "Downloading" })).resolves.toBeDefined();
});

const failed = yours(RequestState.FAILED, { failureReason: "Radarr is unreachable" });

test("a Failed Request says why on hover and focus, and offers no Retry to its Requester", async () => {
  openApp("/movies/tmdb-438631", server(failed));

  const pill = await screen.findByRole("button", { name: "Failed" });

  expect(screen.getByRole("button", { description: "Radarr is unreachable" })).toBe(pill);
  fireEvent.focus(pill);
  await expect(screen.findByRole("tooltip")).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Retry Dune" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Request Dune" })).toBeNull();
});

test("a Request manager retries a Failed Request from the title", async () => {
  const retried: bigint[] = [];
  openApp("/movies/tmdb-438631", server(failed, [...newUser, Permission.MANAGE_REQUESTS], retried));

  fireEvent.click(await screen.findByRole("button", { name: "Retry Dune" }));

  await waitFor(() => {
    expect(retried).toStrictEqual([5n]);
  });
});

test("a Season of yours Declined can be picked and Requested again, and one Failed can't", async () => {
  const show = create(FoundShowSchema, {
    id: { namespace: "tmdb", value: "70523" },
    title: "Dark",
    seasons: [
      { seasonNumber: 1, request: yours(RequestState.DECLINED) },
      {
        seasonNumber: 2,
        request: yours(RequestState.FAILED, { requesters: [{ userId: 2n, name: "Ada" }] }),
      },
    ],
  });
  openApp(
    "/shows/tmdb-70523",
    fakeServer({ signedIn: newUser }, {}, {}, (router) => {
      router.service(RequestService, { listRequests: () => ({}), lookUpShow: () => ({ show }) });
    }),
  );

  const declined = await screen.findByRole<HTMLInputElement>("checkbox", { name: "Season 1" });

  expect(declined.disabled).toBe(false);
  expect(screen.getByRole<HTMLInputElement>("checkbox", { name: "Season 2" }).disabled).toBe(true);
});

test("a Request manager retries a Failed Season from its row", async () => {
  const retried: bigint[] = [];
  const show = create(FoundShowSchema, {
    id: { namespace: "tmdb", value: "70523" },
    title: "Dark",
    seasons: [{ seasonNumber: 2, request: failed }],
  });
  openApp(
    "/shows/tmdb-70523",
    fakeServer({ signedIn: [...newUser, Permission.MANAGE_REQUESTS] }, {}, {}, (router) => {
      router.service(RequestService, {
        listRequests: () => ({}),
        lookUpShow: () => ({ show }),
        retryRequest: ({ id }) => {
          retried.push(id);
          return {};
        },
      });
    }),
  );

  const season2 = await screen.findByRole("checkbox", { name: "Season 2" });
  fireEvent.click(screen.getByRole("button", { name: "Retry Season 2 of Dark" }));

  await waitFor(() => {
    expect(retried).toStrictEqual([5n]);
  });
  expect(season2.getAttribute("aria-describedby")).not.toBeNull();
  expect(screen.getByRole("checkbox", { description: "Failed" })).toBe(season2);
});

test("a Movie to Request shows its Score, genres and Content Rating, its Original audio in Details, and its Trailers", async () => {
  openApp("/movies/tmdb-438631", server({}));

  const score = await screen.findByText("Score");
  expect(score.closest("p")?.textContent).toBe("Score7.82021PG-13");
  expect(screen.getByText("Movie · Science Fiction · Adventure")).toBeDefined();
  const audio = screen.getByText("Original audio", { selector: "dt" });
  expect(audio.nextElementSibling?.textContent).toBe("English");
  expect(screen.getByRole("link", { name: "Official Trailer" }).getAttribute("href")).toBe(
    "https://www.youtube.com/watch?v=n9xhJrPXop4",
  );
});

test("a Movie to Request shows who stars in it, its Cast & Crew, and who directed it", async () => {
  openApp("/movies/tmdb-438631", server({}));

  await expect(screen.findByText("Starring Timothée Chalamet")).resolves.toBeDefined();
  const row = within(screen.getByRole("list", { name: "Cast & Crew" }));
  expect(row.getAllByRole("listitem")).toHaveLength(2);
  const directed = screen.getByText("Directed by", { selector: "dt" });
  expect(directed.nextElementSibling?.textContent).toBe("Denis Villeneuve");
});

test("a Movie to Request goes on your Watchlist with its button, then shows it's on it", async () => {
  const dune = create(FoundMovieSchema, {
    id: { namespace: "tmdb", value: "438631" },
    title: "Dune",
  });
  const added: (string | undefined)[] = [];
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({}),
      lookUpMovie: () => ({ movie: dune }),
    });
    router.service(WatchlistService, {
      addToWatchlist: ({ id }) => {
        added.push(id?.value);
        dune.onWatchlist = true;
        return {};
      },
    });
  });
  openApp("/movies/tmdb-438631", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Add to your Watchlist" }));

  await screen.findByRole("button", { name: "Remove from your Watchlist" });
  expect(added).toStrictEqual(["438631"]);
});
