import { create } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import {
  FollowedShowSchema,
  RequestSchema,
  RequestService,
  RequestState,
  RequestedQualitySchema,
} from "@/gen/nama/v1/request_pb";

import { admin, fakeServer, openApp } from "./test-app";

afterEach(cleanup);

/**
 * A fake Server holding Dark's Pending Followed Show at 4K and its Season 1 Request, that approves
 * the Followed Show, at HD if asked, and cancels it.
 */
function followedServer() {
  const followed = create(FollowedShowSchema, {
    id: 7n,
    showTmdbId: 70_523n,
    showTitle: "Dark",
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
    quality: { id: 2n, name: "4K", profileId: "5" },
  });
  const season1 = create(RequestSchema, {
    id: 3n,
    movieOrSeason: {
      case: "season",
      value: { showTmdbId: 70_523n, seasonNumber: 1, showTitle: "Dark" },
    },
    state: RequestState.PENDING,
    requesters: [{ userId: 2n, name: "Ada" }],
    followedShowId: 7n,
  });
  let followedShows = [followed];
  return fakeServer({ signedIn: admin }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({ requests: [season1], followedShows }),
      countPendingRequests: () => ({}),
      listOfferedQualities: () => ({
        qualities: [
          { id: 1n, name: "HD" },
          { id: 2n, name: "4K" },
        ],
      }),
      approveFollowedShow: ({ qualityId }) => {
        if (qualityId === 1n) {
          followed.quality = create(RequestedQualitySchema, { id: 1n, name: "HD", profileId: "4" });
        }
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

test("a Request manager approves a 4K Followed Show at the other Quality, HD", async () => {
  openApp("/requests", followedServer());
  expect(await screen.findByText("4K · Requested by Ada")).toBeDefined();

  fireEvent.click(
    await screen.findByRole("button", {
      name: "Approve the Followed Show for Dark at another Quality",
    }),
  );
  fireEvent.click(await screen.findByRole("menuitem", { name: "HD" }));

  expect(await screen.findByText("HD · Requested by Ada")).toBeDefined();
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
