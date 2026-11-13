import { create } from "@bufbuild/protobuf";
import { cleanup, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { RequestSchema, RequestService, RequestState } from "@/gen/nama/v1/request_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

/** Dark's Season 1, which the Server failed because Sonarr needs an id in `namespaces`. */
function lackingIds(namespaces: string) {
  return create(RequestSchema, {
    id: 1n,
    movieOrSeason: {
      case: "season",
      value: { showId: { namespace: "tmdb", value: "70523" }, seasonNumber: 1, showTitle: "Dark" },
    },
    state: RequestState.FAILED,
    failureReason: `sonarr needs the Show's ${namespaces} id, and none is known for it`,
    failure: {
      reason: RefusalReason.TITLE_ID_MISSING,
      params: { pluginId: "sonarr", namespaces },
    },
    requesters: [{ userId: 2n, name: "Ada" }],
  });
}

function serving(...requests: ReturnType<typeof lackingIds>[]) {
  return fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({ requests }),
      countPendingRequests: () => ({}),
    });
  });
}

test("a Request the Server didn't hand off for lack of an id says which, in the app's words", async () => {
  openApp("/requests", serving(lackingIds("tvdb")));

  expect(
    await screen.findByText("Reason: Sonarr can’t fetch it: no TVDB id is known for this Show."),
  ).toBeDefined();
});

test("a Request lacking every id of several its Fulfiller needs names each", async () => {
  openApp("/requests", serving(lackingIds("tvdb,imdb")));

  expect(
    await screen.findByText(
      "Reason: Sonarr can’t fetch it: no TVDB or IMDB id is known for this Show.",
    ),
  ).toBeDefined();
});
