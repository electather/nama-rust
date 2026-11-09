import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  FollowedShowSchema,
  type Request,
  RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";

import { admin, fakeServer, newUser, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

const heat: MessageInitShape<typeof RequestSchema> = {
  id: 2n,
  movie: { tmdbId: 949n, title: "Heat" },
  state: RequestState.PENDING,
  requesters: [{ userId: 2n, name: "Ada" }],
};

const dune: MessageInitShape<typeof RequestSchema> = {
  id: 1n,
  movie: { tmdbId: 438_631n, title: "Dune" },
  state: RequestState.APPROVED,
  requesters: [{ userId: 3n, name: "Grace" }],
};

const alien: MessageInitShape<typeof RequestSchema> = {
  id: 4n,
  movie: { tmdbId: 348n, title: "Alien" },
  state: RequestState.FAILED,
  failureReason: "Radarr is unreachable",
  requesters: [{ userId: 2n, name: "Ada" }],
};

/**
 * A fake Server holding `requests`, signed in with `permissions`, that approves, declines,
 * cancels and retries them, the Fulfiller taking a retried one. It changes the messages given, so
 * a test can change them too, as someone else would.
 */
function managedServer(permissions: Permission[], ...requests: Request[]) {
  const held = new Map(requests.map((request) => [request.id, request]));
  const decide = (id: bigint, state: RequestState, declineReason?: string) => {
    const request = held.get(id) ?? create(RequestSchema);
    if (request.state !== RequestState.PENDING) {
      throw refused(Code.FailedPrecondition, "it isn't Pending", {
        reason: RefusalReason.REQUEST_NOT_PENDING,
      });
    }
    Object.assign(request, { state, declineReason });
    return { request };
  };
  return fakeServer({ signedIn: permissions }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: ({ state }) => ({
        requests: [...held.values()].filter(
          (request) => state === RequestState.UNSPECIFIED || request.state === state,
        ),
      }),
      approveRequest: ({ id }) => decide(id, RequestState.APPROVED),
      declineRequest: ({ id, reason }) => decide(id, RequestState.DECLINED, reason),
      cancelRequest: ({ id }) => {
        held.delete(id);
        return {};
      },
      retryRequest: ({ id }) => {
        const request = held.get(id) ?? create(RequestSchema);
        if (request.state !== RequestState.FAILED) {
          throw refused(Code.FailedPrecondition, "it isn't Failed", {
            reason: RefusalReason.REQUEST_NOT_FAILED,
          });
        }
        Object.assign(request, { state: RequestState.PROCESSING, failureReason: undefined });
        return { request };
      },
    });
  });
}

test("a User without Manage Requests isn't offered Approve, Decline, Cancel or Retry", async () => {
  openApp(
    "/requests",
    managedServer(newUser, create(RequestSchema, heat), create(RequestSchema, alien)),
  );

  await screen.findByText("Heat");
  expect(screen.getByText("Reason: Radarr is unreachable")).toBeDefined();
  expect(screen.queryByRole("button", { name: /^(Approve|Decline|Cancel|Retry)/u })).toBeNull();
});

test("approving a Pending Request shows it Approved", async () => {
  openApp("/requests", managedServer(admin, create(RequestSchema, heat)));

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  expect(await screen.findByText("Approved")).toBeDefined();
  expect(screen.queryByRole("button", { name: "Approve Heat" })).toBeNull();
});

test("approving a Request while showing Pending ones takes it off the list", async () => {
  openApp(
    "/requests?state=pending",
    managedServer(admin, create(RequestSchema, heat), create(RequestSchema, dune)),
  );

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  expect(await screen.findByText("No Pending Requests.")).toBeDefined();
  expect(screen.queryByText("Dune")).toBeNull();
});

test("a Season Request's actions name its Show and Season", async () => {
  const specials = create(RequestSchema, {
    id: 3n,
    season: { showTmdbId: 70_523n, seasonNumber: 0, showTitle: "Dark" },
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
  });
  openApp("/requests", managedServer(admin, specials));

  fireEvent.click(await screen.findByRole("button", { name: "Approve Dark Specials" }));

  expect(await screen.findByText("Approved")).toBeDefined();
});

test("declining a Request asks why, then shows it Declined with the reason", async () => {
  openApp("/requests", managedServer(admin, create(RequestSchema, heat)));

  fireEvent.click(await screen.findByRole("button", { name: "Decline Heat" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Decline Heat?" });
  fireEvent.change(within(dialog).getByLabelText("Reason (optional)"), {
    target: { value: "Not out on disc yet" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "Decline" }));

  expect(await screen.findByText("Reason: Not out on disc yet")).toBeDefined();
  expect(screen.getByText("Declined")).toBeDefined();
});

test("cancelling a Request, once confirmed, removes it", async () => {
  openApp("/requests", managedServer(admin, create(RequestSchema, dune)));

  fireEvent.click(await screen.findByRole("button", { name: "Cancel Request for Dune" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Cancel the Request for Dune?" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel Request" }));

  expect(await screen.findByText("No Requests yet.")).toBeDefined();
});

test("approving a Request someone else declined meanwhile says so, and shows it Declined", async () => {
  const request = create(RequestSchema, heat);
  openApp("/requests", managedServer(admin, request));
  const approve = await screen.findByRole("button", { name: "Approve Heat" });

  request.state = RequestState.DECLINED;
  fireEvent.click(approve);

  expect(await screen.findByText("Heat was already approved or declined.")).toBeDefined();
  expect(await screen.findByText("Declined")).toBeDefined();
});

test("retrying a Failed Request shows it Processing, without its reason", async () => {
  openApp("/requests", managedServer(admin, create(RequestSchema, alien)));

  fireEvent.click(await screen.findByRole("button", { name: "Retry Alien" }));

  expect(await screen.findByText("Processing")).toBeDefined();
  expect(screen.queryByText("Reason: Radarr is unreachable")).toBeNull();
  expect(screen.queryByRole("button", { name: "Retry Alien" })).toBeNull();
});

test("retrying a Request that stopped being Failed meanwhile says so", async () => {
  const request = create(RequestSchema, alien);
  openApp("/requests", managedServer(admin, request));
  const retry = await screen.findByRole("button", { name: "Retry Alien" });

  request.state = RequestState.PROCESSING;
  fireEvent.click(retry);

  expect(await screen.findByText("Alien isn’t Failed any more.")).toBeDefined();
  expect(await screen.findByText("Processing")).toBeDefined();
});

/**
 * A fake Server holding Ada's Pending Followed Show of Dark, and the Season 1 Request it opened,
 * signed in as an Admin, that approves and cancels the Followed Show.
 */
function followedServer() {
  const followed = create(FollowedShowSchema, {
    id: 7n,
    showTmdbId: 70_523n,
    showTitle: "Dark",
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
  });
  const season1 = create(RequestSchema, {
    id: 3n,
    season: { showTmdbId: 70_523n, seasonNumber: 1, showTitle: "Dark" },
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
    followedShowId: 7n,
  });
  let followedShows = [followed];
  return fakeServer({ signedIn: admin }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({ requests: [season1], followedShows }),
      approveFollowedShow: () => {
        followed.state = RequestState.APPROVED;
        season1.state = RequestState.APPROVED;
        return { followedShow: followed };
      },
      cancelFollowedShow: () => {
        followedShows = [];
        season1.followedShowId = undefined;
        return {};
      },
    });
  });
}

test("approving a Followed Show once approves it and its Season Requests", async () => {
  openApp("/requests", followedServer());

  fireEvent.click(
    await screen.findByRole("button", { name: "Approve the Followed Show for Dark" }),
  );

  await waitFor(() => {
    expect(screen.getAllByText("Approved")).toHaveLength(2);
  });
  expect(screen.queryByRole("button", { name: /^Approve/u })).toBeNull();
});

test("cancelling a Followed Show, once confirmed, removes it and leaves its Season Requests", async () => {
  openApp("/requests", followedServer());

  fireEvent.click(await screen.findByRole("button", { name: "Cancel Followed Show for Dark" }));
  const dialog = await screen.findByRole("alertdialog", {
    name: "Cancel the Followed Show for Dark?",
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel Followed Show" }));

  await waitFor(() => {
    expect(screen.queryByText("Followed Show")).toBeNull();
  });
  expect(screen.getByText("Season 1")).toBeDefined();
});
