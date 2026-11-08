import { type MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { type RequestSchema, RequestService, RequestState } from "@/gen/nama/v1/request_pb";

import { fakeServer, newUser, openApp } from "./test-app";

afterEach(cleanup);

const ada = { userId: 2n, name: "Ada" };
const you = { userId: 1n, name: "Sam" };

const heat = {
  id: 2n,
  movie: { tmdbId: 949n, title: "Heat" },
  state: RequestState.PENDING,
  requesters: [you, ada],
};

/** A fake Server holding `requests`, where withdrawing leaves Heat to Ada alone. */
function server(...requests: MessageInitShape<typeof RequestSchema>[]) {
  let held = requests;
  return fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(RequestService, {
      listRequests: () => ({ requests: held }),
      withdrawRequest: () => {
        held = [{ ...heat, requesters: [ada] }];
        return {};
      },
    });
  });
}

test("withdrawing from your Pending Request takes you off it", async () => {
  openApp("/requests", server(heat));

  fireEvent.click(await screen.findByRole("button", { name: "Withdraw from Heat" }));

  expect(await screen.findByText("Requested by Ada")).toBeDefined();
  expect(screen.queryByRole("button", { name: "Withdraw from Heat" })).toBeNull();
});

test("Withdraw is offered only on your own Pending Requests", async () => {
  openApp(
    "/requests",
    server(
      { ...heat, requesters: [ada] },
      { ...heat, id: 1n, movie: { title: "Dune" }, state: RequestState.APPROVED },
    ),
  );

  await screen.findByText("Heat");
  expect(screen.queryByRole("button", { name: /^Withdraw/u })).toBeNull();
});
