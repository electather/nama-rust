import { type MessageInitShape } from "@bufbuild/protobuf";
import { act, cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import {
  type ListRequestsRequest,
  type RequestSchema,
  RequestService,
  RequestState,
} from "@/gen/nama/v1/request_pb";

import { fakeServer, newUser, openApp, rowTitles } from "./test-app";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const sam = [{ userId: 1n, name: "Sam" }];

function movie(id: bigint, title: string) {
  return {
    id,
    movieOrSeason: { case: "movie" as const, value: { tmdbId: id, title } },
    state: RequestState.PENDING,
    requesters: sam,
  };
}

function darkSeason(id: bigint, seasonNumber: number) {
  const season = { showTmdbId: 70_523n, seasonNumber, showTitle: "Dark" };
  return {
    id,
    movieOrSeason: { case: "season" as const, value: season },
    state: RequestState.APPROVED,
    requesters: sam,
  };
}

/**
 * A fake Server listing `pages` in turn, whatever the state, with each page's index as the token
 * that lists it. Each `ListRequests` it gets goes in `asked`.
 */
function pagingServer(
  pages: MessageInitShape<typeof RequestSchema>[][],
  asked: ListRequestsRequest[] = [],
) {
  return fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: (request) => {
        asked.push(request);
        const index = Number(request.pageToken || "0");
        const next = index + 1 < pages.length ? String(index + 1) : "";
        return { requests: pages[index] ?? [], nextPageToken: next };
      },
    });
  });
}

test("Show more lists the next page below the first, and goes after the last", async () => {
  openApp("/requests", pagingServer([[movie(3n, "Heat")], [movie(2n, "Dune")]]));

  fireEvent.click(await screen.findByRole("button", { name: "Show more" }));

  await screen.findByText("Dune");
  expect(rowTitles()).toEqual(["Heat", "Dune"]);
  expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
});

/**
 * Stands in for the browser's `IntersectionObserver`, with the end of the list in view from the
 * start if `inView`. Returns what brings it into view later.
 */
function stubObserver(inView: boolean) {
  let seen: ((entries: { isIntersecting: boolean }[]) => void) | undefined;
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      constructor(callback: typeof seen) {
        seen = callback;
      }
      observe() {
        if (inView) seen?.([{ isIntersecting: true }]);
      }
      disconnect() {}
    },
  );
  return () => seen?.([{ isIntersecting: true }]);
}

test("scrolling to the end of the list loads the next page", async () => {
  const scrollToEnd = stubObserver(false);
  openApp("/requests", pagingServer([[movie(3n, "Heat")], [movie(2n, "Dune")]]));
  await screen.findByText("Heat");

  act(scrollToEnd);

  expect(await screen.findByText("Dune")).toBeDefined();
});

test("a page that leaves the end of the list in view loads the next one too", async () => {
  stubObserver(true);

  openApp(
    "/requests",
    pagingServer([[movie(3n, "Heat")], [movie(2n, "Dune")], [movie(1n, "Alien")]]),
  );

  await screen.findByText("Alien");
  expect(rowTitles()).toEqual(["Heat", "Dune", "Alien"]);
});

test("a Show's older Season Request joins its group when a later page loads", async () => {
  openApp("/requests", pagingServer([[darkSeason(4n, 2), movie(3n, "Heat")], [darkSeason(2n, 1)]]));

  fireEvent.click(await screen.findByRole("button", { name: "Show more" }));

  await screen.findByText("Season 1");
  expect(rowTitles()).toEqual(["Dark", "Season 1", "Season 2", "Heat"]);
});

test("changing the filter lists the newest of the new state from the first page", async () => {
  const asked: ListRequestsRequest[] = [];
  openApp("/requests", pagingServer([[movie(3n, "Heat")], [movie(2n, "Dune")]], asked));
  fireEvent.click(await screen.findByRole("button", { name: "Show more" }));
  await screen.findByText("Dune");

  fireEvent.click(screen.getByRole("button", { name: "State: All" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Pending" }));

  await waitFor(() => {
    expect(rowTitles()).toEqual(["Heat"]);
  });
  expect(asked.at(-1)).toMatchObject({
    state: RequestState.PENDING,
    pageSize: 50,
    pageToken: "",
  });
});
