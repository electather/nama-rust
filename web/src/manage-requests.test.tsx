import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
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

/**
 * A fake Server holding `requests`, signed in with `permissions`, that approves, declines and
 * cancels them. It changes the messages given, so a test can change them too, as someone else
 * would.
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
      listRequests: () => ({ requests: [...held.values()] }),
      approveRequest: ({ id }) => decide(id, RequestState.APPROVED),
      declineRequest: ({ id, reason }) => decide(id, RequestState.DECLINED, reason),
      cancelRequest: ({ id }) => {
        held.delete(id);
        return {};
      },
    });
  });
}

test("a User without Manage Requests isn't offered Approve, Decline or Cancel", async () => {
  openApp("/requests", managedServer(newUser, create(RequestSchema, heat)));

  await screen.findByText("Heat");
  expect(screen.queryByRole("button", { name: /^(Approve|Decline|Cancel)/u })).toBeNull();
});

test("approving a Pending Request shows it Approved", async () => {
  openApp("/requests", managedServer(admin, create(RequestSchema, heat)));

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat" }));

  expect(await screen.findByText("Approved")).toBeDefined();
  expect(screen.queryByRole("button", { name: "Approve Heat" })).toBeNull();
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
