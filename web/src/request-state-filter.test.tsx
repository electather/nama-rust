import { type MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import {
  type FollowedShowSchema,
  type RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import type { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, eventFeed, fakeServer, newUser, openApp, rowTitles } from "./test-app";

afterEach(cleanup);

const ada = [{ userId: 2n, name: "Ada" }];

const pendingHeat = {
  id: 2n,
  movieOrSeason: { case: "movie" as const, value: { tmdbId: 949n, title: "Heat" } },
  state: RequestState.PENDING,
  requesters: ada,
};

const failedDune = {
  id: 1n,
  movieOrSeason: { case: "movie" as const, value: { tmdbId: 438_631n, title: "Dune" } },
  state: RequestState.FAILED,
  requesters: ada,
};

/** Whether a row is in `state`, which lists every row when unspecified, as the Server filters. */
function isIn(row: { state?: RequestState }, state: RequestState) {
  return state === RequestState.UNSPECIFIED || row.state === state;
}

/**
 * A fake Server holding `requests` and `followedShows`, which lists and counts those in the state
 * asked for and approves a Request in `requests`, signed in with `signedIn`.
 */
function filteringServer(
  requests: MessageInitShape<typeof RequestSchema>[],
  followedShows: MessageInitShape<typeof FollowedShowSchema>[] = [],
  events = new ReadableStream<SubscribeResponse>(),
  signedIn: Permission[] = newUser,
) {
  return fakeServer({ signedIn, events }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: ({ state }) => ({
        requests: requests.filter((row) => isIn(row, state)),
        followedShows: followedShows.filter((row) => isIn(row, state)),
      }),
      countPendingRequests: () => ({
        count: [...requests, ...followedShows].filter((row) => row.state === RequestState.PENDING)
          .length,
      }),
      approveRequest: ({ id }) => {
        const index = requests.findIndex((row) => row.id === id);
        requests[index] = { ...requests[index], state: RequestState.APPROVED };
        return { request: requests[index] };
      },
    });
  });
}

test("choosing Failed shows only the Failed Requests and keeps the choice in the URL", async () => {
  openApp("/requests", filteringServer([pendingHeat, failedDune]));

  fireEvent.click(await screen.findByRole("button", { name: "State: All" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Failed" }));

  await waitFor(() => {
    expect(rowTitles()).toEqual(["Dune"]);
  });
  expect(screen.getByRole("button", { name: "State: Failed" })).toBeDefined();
  expect(new URLSearchParams(window.location.search).get("state")).toBe("failed");
});

test("the filter offers All and every state, in the order Requests move through them", async () => {
  openApp("/requests", filteringServer([]));

  fireEvent.click(await screen.findByRole("button", { name: "State: All" }));

  const offered = await screen.findAllByRole("menuitem");
  expect(offered.map((item) => item.textContent)).toEqual([
    "All",
    "Pending",
    "Approved",
    "Processing",
    "Failed",
    "Partially Available",
    "Available",
    "Declined",
  ]);
});

test("a link to the Requests page with a state opens on that state", async () => {
  openApp("/requests?state=failed", filteringServer([pendingHeat, failedDune]));

  expect(await screen.findByRole("button", { name: "State: Failed" })).toBeDefined();
  expect(rowTitles()).toEqual(["Dune"]);
});

test("a state the app doesn't know in the URL shows All", async () => {
  openApp("/requests?state=lost", filteringServer([pendingHeat, failedDune]));

  expect(await screen.findByRole("button", { name: "State: All" })).toBeDefined();
  expect(rowTitles()).toEqual(["Heat", "Dune"]);
});

test("a Failed Season Request shows under its Show's title without its Approved Followed Show", async () => {
  const season = { showTmdbId: 70_523n, seasonNumber: 1, showTitle: "Dark" };
  openApp(
    "/requests?state=failed",
    filteringServer(
      [
        {
          id: 3n,
          movieOrSeason: { case: "season", value: season },
          state: RequestState.FAILED,
          requesters: ada,
          followedShowId: 7n,
        },
      ],
      [
        {
          id: 7n,
          showTmdbId: 70_523n,
          showTitle: "Dark",
          state: RequestState.APPROVED,
          requesters: ada,
        },
      ],
    ),
  );

  const dark = await screen.findByRole("list", { name: "Dark" });
  expect(
    within(dark)
      .getAllByRole("listitem")
      .map((row) => row.textContent),
  ).toEqual(["Season 1Requested by AdaFailed"]);
});

test("the Requests page says so when none are in the chosen state", async () => {
  openApp("/requests?state=processing", filteringServer([pendingHeat]));

  expect(await screen.findByText("No Processing Requests.")).toBeDefined();
});

test("a Request retried elsewhere leaves the Failed ones without a reload", async () => {
  const events = eventFeed();
  const requests: MessageInitShape<typeof RequestSchema>[] = [failedDune];
  openApp("/requests?state=failed", filteringServer(requests, [], events.stream));
  await screen.findByText("Dune");

  requests[0] = { ...failedDune, state: RequestState.PROCESSING };
  events.send({ event: { case: "requestStateChanged", value: { request: requests[0] } } });

  expect(await screen.findByText("No Failed Requests.")).toBeDefined();
});

test("approving a Request while showing Pending ones takes it off the list", async () => {
  const pendingDune = { ...failedDune, state: RequestState.PENDING };
  openApp(
    "/requests?state=pending",
    filteringServer([pendingHeat, pendingDune], [], undefined, admin),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  await waitFor(() => {
    expect(rowTitles()).toEqual(["Dune"]);
  });
});

test.each([
  {
    who: "a Request manager with a Pending Request",
    signedIn: admin,
    lands: "pending",
    requests: [pendingHeat, failedDune],
  },
  {
    who: "a Request manager with nothing Pending",
    signedIn: admin,
    lands: "all",
    requests: [failedDune],
  },
  {
    who: "a User without Manage Requests",
    signedIn: newUser,
    lands: "all",
    requests: [pendingHeat, failedDune],
  },
])("the Requests page opens $who on $lands", async ({ signedIn, lands, requests }) => {
  openApp("/requests", filteringServer(requests, [], undefined, signedIn));

  await waitFor(() => {
    expect(new URLSearchParams(window.location.search).get("state")).toBe(lands);
  });
});

test("a state the app doesn't know opens a Request manager on Pending", async () => {
  openApp("/requests?state=lost", filteringServer([pendingHeat, failedDune], [], undefined, admin));

  await waitFor(() => {
    expect(new URLSearchParams(window.location.search).get("state")).toBe("pending");
  });
});

test("approving the last Pending Request leaves the page on Pending", async () => {
  openApp("/requests", filteringServer([pendingHeat, failedDune], [], undefined, admin));

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  expect(await screen.findByText("No Pending Requests.")).toBeDefined();
  expect(screen.getByRole("button", { name: "State: Pending" })).toBeDefined();
});

test("Back from the Requests page leaves it, past the default it chose", async () => {
  const router = openApp("/settings", filteringServer([pendingHeat], [], undefined, admin));

  fireEvent.click(await screen.findByRole("link", { name: "Requests, 1 Pending" }));
  await screen.findByRole("button", { name: "State: Pending" });
  router.history.back();

  expect(await screen.findByRole("heading", { name: "Settings" })).toBeDefined();
});
