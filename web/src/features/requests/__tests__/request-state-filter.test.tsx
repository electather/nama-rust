import type { MessageInitShape } from "@bufbuild/protobuf";
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
import { admin, eventFeed, fakeServer, newUser, openApp, requestTitles } from "@/test-app";

afterEach(cleanup);

const ada = [{ userId: 2n, name: "Ada" }];

const pendingHeat = {
  id: 2n,
  movieOrSeason: {
    case: "movie" as const,
    value: { id: { namespace: "tmdb", value: "949" }, title: "Heat" },
  },
  state: RequestState.PENDING,
  requesters: ada,
};

const failedDune = {
  id: 1n,
  movieOrSeason: {
    case: "movie" as const,
    value: { id: { namespace: "tmdb", value: "438631" }, title: "Dune" },
  },
  state: RequestState.FAILED,
  requesters: ada,
};

/** Whether a row is in `state`, which lists every row when unspecified, as the Server filters. */
function isIn(row: { state?: RequestState }, state: RequestState = RequestState.UNSPECIFIED) {
  return state === RequestState.UNSPECIFIED || row.state === state;
}

/**
 * A fake Server holding `requests` and `followedShows`, which lists and counts those in the state
 * asked for and approves a Request in `requests`, signed in with `signedIn`.
 */
function filteringServer(
  requests: MessageInitShape<typeof RequestSchema>[],
  followedShows: MessageInitShape<typeof FollowedShowSchema>[] = [],
  options: { events?: ReadableStream<SubscribeResponse>; signedIn?: Permission[] } = {},
) {
  const { events = new ReadableStream<SubscribeResponse>(), signedIn = newUser } = options;
  return fakeServer({ signedIn, events }, {}, {}, (router) => {
    router.service(RequestService, {
      listDefaultQualities: () => ({}),
      listRequests: ({ filter }) => ({
        requests: requests.filter((row) => isIn(row, filter?.state)),
        followedShows: followedShows.filter((row) => isIn(row, filter?.state)),
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

/** Picks `state` from the filter's menu, which says `from` until then. */
async function choose(state: string, from = "All") {
  fireEvent.click(await screen.findByRole("button", { name: `State: ${from}` }));
  fireEvent.click(await screen.findByRole("menuitem", { name: state }));
}

test("choosing Failed shows only the Failed Requests", async () => {
  openApp("/?settings=requests", filteringServer([pendingHeat, failedDune]));

  await choose("Failed");

  await waitFor(() => {
    expect(requestTitles()).toStrictEqual(["Dune"]);
  });
  expect(screen.getByRole("button", { name: "State: Failed" })).toBeDefined();
});

test("the filter offers All and every state, in the order Requests move through them", async () => {
  openApp("/?settings=requests", filteringServer([]));

  fireEvent.click(await screen.findByRole("button", { name: "State: All" }));

  const offered = await screen.findAllByRole("menuitem");
  expect(offered.map((item) => item.textContent)).toStrictEqual([
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

test("a Failed Season Request shows under its Show's title without its Approved Followed Show", async () => {
  const season = {
    showId: { namespace: "tmdb", value: "70523" },
    seasonNumber: 1,
    showTitle: "Dark",
  };
  openApp(
    "/?settings=requests",
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
          showId: { namespace: "tmdb", value: "70523" },
          showTitle: "Dark",
          state: RequestState.APPROVED,
          requesters: ada,
        },
      ],
    ),
  );
  await choose("Failed");

  // The unfiltered "Dark" list, with its Followed Show row, shows until the Failed refetch lands.
  await waitFor(() => {
    expect(
      within(screen.getByRole("list", { name: "Dark" }))
        .getAllByRole("listitem")
        .map((row) => row.textContent),
    ).toStrictEqual(["Season 1FailedRequested by Ada"]);
  });
});

test("the Requests page says so when none are in the chosen state", async () => {
  openApp("/?settings=requests", filteringServer([pendingHeat]));
  await choose("Processing");

  await expect(screen.findByText("No Processing Requests.")).resolves.toBeDefined();
});

test("a Request retried elsewhere leaves the Failed ones without a reload", async () => {
  const events = eventFeed();
  const requests: MessageInitShape<typeof RequestSchema>[] = [failedDune];
  openApp("/?settings=requests", filteringServer(requests, [], { events: events.stream }));
  await choose("Failed");
  await screen.findByText("Dune");

  requests[0] = { ...failedDune, state: RequestState.PROCESSING };
  events.send({ event: { case: "requestStateChanged", value: { request: requests[0] } } });

  await expect(screen.findByText("No Failed Requests.")).resolves.toBeDefined();
});

test("approving a Request while showing Pending ones takes it off the list", async () => {
  const pendingDune = { ...failedDune, state: RequestState.PENDING };
  openApp(
    "/?settings=requests",
    filteringServer([pendingHeat, pendingDune], [], { signedIn: admin }),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  await waitFor(() => {
    expect(requestTitles()).toStrictEqual(["Dune"]);
  });
});

test.each([
  {
    who: "a Request manager with a Pending Request",
    signedIn: admin,
    lands: "Pending",
    requests: [pendingHeat, failedDune],
  },
  {
    who: "a Request manager with nothing Pending",
    signedIn: admin,
    lands: "All",
    requests: [failedDune],
  },
  {
    who: "a User without Manage Requests",
    signedIn: newUser,
    lands: "All",
    requests: [pendingHeat, failedDune],
  },
])("requests open $who on $lands", async ({ signedIn, lands, requests }) => {
  openApp("/?settings=requests", filteringServer(requests, [], { signedIn }));

  await expect(screen.findByRole("button", { name: `State: ${lands}` })).resolves.toBeDefined();
});

test("approving the last Pending Request leaves the page on Pending", async () => {
  openApp(
    "/?settings=requests",
    filteringServer([pendingHeat, failedDune], [], { signedIn: admin }),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  await expect(screen.findByText("No Pending Requests.")).resolves.toBeDefined();
  expect(screen.getByRole("button", { name: "State: Pending" })).toBeDefined();
});
