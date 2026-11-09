import { type MessageInitShape, create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  QualityChangeSchema,
  type Request,
  RequestedQualitySchema,
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

/** The Movie Fulfiller's Qualities, as an admin offered them. */
const hdAnd4k = [
  { id: 1n, name: "HD", isDefault: true },
  { id: 2n, name: "4K" },
];

/** The offered Quality `name`, with the id `id`, as a Request copies it. */
function requested(name: string, id: bigint) {
  return create(RequestedQualitySchema, { id, name, profileId: String(id) });
}

/** When Sam changes a Quality: midday, so the day is 9 Nov 2026 in any time zone. */
const changedAt = BigInt(Date.UTC(2026, 10, 9, 12) / 1000);

/** Changes `request`'s Quality to the offered one with the id `qualityId`, as Sam, if given. */
function changeQuality(request: Request, qualityId: bigint | undefined) {
  const quality = hdAnd4k.find(({ id }) => id === qualityId);
  if (!quality) return;
  request.qualityChange = create(QualityChangeSchema, {
    changedBy: "Sam",
    changedAt,
    ...(request.quality && { previousName: request.quality.name }),
  });
  request.quality = requested(quality.name, quality.id);
}

/**
 * A fake Server holding `requests`, signed in with `permissions`, that approves, declines,
 * cancels and retries them, at another of {@link hdAnd4k} if asked, the Fulfiller taking a
 * retried one. It changes the messages given, so a test can change them too, as someone else
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
      countPendingRequests: () => ({}),
      listOfferedQualities: () => ({ qualities: hdAnd4k }),
      approveRequest: ({ id, qualityId }) => {
        const request = held.get(id);
        if (request?.state === RequestState.PENDING) changeQuality(request, qualityId);
        return decide(id, RequestState.APPROVED);
      },
      declineRequest: ({ id, reason }) => decide(id, RequestState.DECLINED, reason),
      cancelRequest: ({ id }) => {
        held.delete(id);
        return {};
      },
      retryRequest: ({ id, qualityId }) => {
        const request = held.get(id) ?? create(RequestSchema);
        if (request.state !== RequestState.FAILED) {
          throw refused(Code.FailedPrecondition, "it isn't Failed", {
            reason: RefusalReason.REQUEST_NOT_FAILED,
          });
        }
        changeQuality(request, qualityId);
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

test("a Season Request's actions name its Show and Season, and approve it at another Quality", async () => {
  const specials = create(RequestSchema, {
    id: 3n,
    season: { showTmdbId: 70_523n, seasonNumber: 0, showTitle: "Dark" },
    state: RequestState.PENDING,
  });
  openApp("/requests", managedServer(admin, specials));

  fireEvent.click(await screen.findByRole("button", { name: /^Approve Dark Specials at/u }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "HD" }));

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

test("a Request manager approves a 4K Request at the other Quality, HD, and it says who changed it when", async () => {
  const heat4k = create(RequestSchema, { ...heat, quality: requested("4K", 2n) });
  openApp("/requests", managedServer(admin, heat4k));

  fireEvent.click(await screen.findByRole("button", { name: "Approve Heat at another Quality" }));
  const others = await screen.findAllByRole("menuitem");
  fireEvent.click(await screen.findByRole("menuitem", { name: "HD" }));

  expect(others.map((item) => item.textContent)).toStrictEqual(["HD"]);
  expect(await screen.findByText("Approved")).toBeDefined();
  expect(screen.getByText(/HD, changed from 4K by Sam on .*9.*2026/u)).toBeDefined();
  expect(screen.queryByRole("button", { name: /^Approve Heat/u })).toBeNull();
});

test("a Request manager retries a Failed Request at another Quality", async () => {
  const alienHd = create(RequestSchema, { ...alien, quality: requested("HD", 1n) });
  openApp("/requests", managedServer(admin, alienHd));

  fireEvent.click(await screen.findByRole("button", { name: "Retry Alien at another Quality" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "4K" }));

  expect(await screen.findByText("Processing")).toBeDefined();
  expect(screen.getByText(/4K, changed from HD by Sam on .*9.*2026/u)).toBeDefined();
});

/**
 * A fake Server holding Ada's Pending Followed Show of Dark, and the Season 1 Request it opened,
 * signed in as an Admin, that approves and cancels the Followed Show.
 */
