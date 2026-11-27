import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type Transport, createRouterTransport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";
import { cleanup, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vite-plus/test";

import type { IdentityChange } from "@/features/auth/data/use-identity-change";
import { serverSessionKey } from "@/features/servers/lib/server-sessions";
import {
  LinkService,
  LinkedServerSchema,
  ListLinkedServersResponseSchema,
} from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { WebPushService } from "@/gen/nama/v1/web_push_pb";
import { signedInAccount, whoIsSignedIn } from "@/shared/hooks/access";
import type { ServerAt } from "@/shared/lib/server";
import { fakeServer, newUser } from "@/test-app";
import { accountHub, ada } from "@/test-hub";
import { hubRefused } from "@/test-hub-links";
import {
  type ChangeAt,
  cachedFromBefore,
  keptSessionKeys,
  openIdentityChange,
} from "@/test-identity-change";
import { browserWithPush, forgetBrowserPush } from "@/test-web-push";

// `web/src/AGENTS.md`, "One module changes who is signed in": a row per kind of change, asserting
// what it did outside, in order, in `log`.

afterEach(() => {
  cleanup();
  forgetBrowserPush();
  localStorage.clear();
});

/** The Device a Server's change runs on: its Users, and whether its Server refuses push. */
interface Device {
  deviceUsers: bigint[];
  refusesPush: boolean;
}

/**
 * A Server whose Device acts as Sam, among `deviceUsers`, noting in `log` this browser's push
 * subscription moving to or leaving the User signed in, or refusing it while `refusesPush`.
 */
function serverWithPush(log: string[], device: Device) {
  return fakeServer(
    { signedIn: newUser },
    {
      whoAmI: () => ({
        user: { id: 1n, name: "Sam", permissions: newUser },
        deviceUsers: device.deviceUsers.map((id) => ({ id, name: `User ${id}` })),
      }),
    },
    {},
    (router) => {
      router.service(WebPushService, {
        subscribeWebPush: () => {
          if (device.refusesPush) {
            throw new ConnectError("database is locked", Code.Internal);
          }
          log.push("push moved");
          return {};
        },
        unsubscribeWebPush: () => {
          log.push("push left");
          return {};
        },
      });
    },
  );
}

/** A call that notes in `log` where it was made, then fails with `refusal`, if any. */
function callNoting(log: string[], at: ChangeAt, refusal?: ConnectError) {
  // oxlint-disable-next-line require-await, typescript/require-await -- the call stands in for an RPC, and this one answers at once
  return async (transport: Transport) => {
    log.push(
      transport === at.hub && at.hub !== at.server ? "call at the Hub" : "call at the Server",
    );
    if (refusal) {
      throw refusal;
    }
  };
}

const notFound = new ConnectError("no such User", Code.NotFound);

/** Where a change runs at a Server, its page's origin, on a Device with two Users by default. */
function atServer(log: string[], device?: Device) {
  const server = serverWithPush(log, device ?? { deviceUsers: [1n, 2n], refusesPush: false });
  const serverAt: ServerAt = () => server;
  return { server, hub: server, serverAt };
}

describe("at a Server", () => {
  let browser: ReturnType<typeof browserWithPush>;
  beforeEach(() => {
    browser = browserWithPush("granted", true);
  });

  const succeeding: [string, IdentityChange<unknown>, string[]][] = [
    [
      "a sign-in, by any way, moves push, clears the cache, and goes on in its page's place",
      { kind: "signed in", goOn: "/next" },
      ["call at the Server", "push moved", "cache cleared", "replaced with /next"],
    ],
    [
      "a switch moves push, clears the cache, and goes on in its page's place",
      { kind: "switched", goOn: "/next" },
      ["call at the Server", "push moved", "cache cleared", "replaced with /next"],
    ],
    [
      "joining with an Invite or setting up goes Home in its page's place",
      { kind: "signed in", goOn: undefined },
      ["call at the Server", "push moved", "cache cleared", "replaced with /"],
    ],
    [
      "signing the User out leaves push first, then opens the User picker",
      { kind: "user signed out" },
      ["push left", "call at the Server", "cache cleared", "went to /switch-user"],
    ],
    [
      "signing the Device out leaves push first, then opens sign-in",
      { kind: "device signed out" },
      ["push left", "call at the Server", "cache cleared", "went to /sign-in"],
    ],
  ];

  test.each(succeeding)("%s", async (_name, change, expected) => {
    const log: string[] = [];
    const at = atServer(log);
    const { change: make, queryClient } = await openIdentityChange(change, at, log);

    await make(callNoting(log, at));

    expect(log).toStrictEqual(expected);
    expect(cachedFromBefore(queryClient)).toBeUndefined();
  });

  test("leaving push stops it in the browser too", async () => {
    const log: string[] = [];
    const at = atServer(log);
    const { change } = await openIdentityChange({ kind: "user signed out" }, at, log);

    await change(callNoting(log, at));

    expect(browser.subscription.unsubscribe).toHaveBeenCalledWith();
  });

  test("a sign-in whose Server refuses push unsubscribes the browser, and finishes", async () => {
    const log: string[] = [];
    const at = atServer(log, { deviceUsers: [1n], refusesPush: true });
    const { change } = await openIdentityChange({ kind: "signed in", goOn: "/next" }, at, log);

    await change(callNoting(log, at));

    expect(log).toStrictEqual(["call at the Server", "cache cleared", "replaced with /next"]);
    expect(browser.subscription.unsubscribe).toHaveBeenCalledWith();
  });

  const failing = new ConnectError("database is locked", Code.Internal);

  test.each<[string, IdentityChange<unknown>]>([
    ["a sign-in", { kind: "signed in", goOn: "/next" }],
    ["a switch", { kind: "switched", goOn: "/next" }],
  ])("%s whose call fails rethrows, and does nothing after it", async (_name, kind) => {
    const log: string[] = [];
    const at = atServer(log);
    const { change, router, queryClient } = await openIdentityChange(kind, at, log);

    await expect(change(callNoting(log, at, failing))).rejects.toBe(failing);

    expect(log).toStrictEqual(["call at the Server"]);
    expect(cachedFromBefore(queryClient)).toBe("theirs");
    expect(router.state.location.pathname).toBe("/start");
  });

  test.each<[string, IdentityChange<unknown>]>([
    ["signing the User out", { kind: "user signed out" }],
    ["signing the Device out", { kind: "device signed out" }],
  ])("%s, when its call fails, rethrows, having left push all the same", async (_name, kind) => {
    const log: string[] = [];
    const at = atServer(log);
    const { change, queryClient } = await openIdentityChange(kind, at, log);

    await expect(change(callNoting(log, at, failing))).rejects.toBe(failing);

    expect(log).toStrictEqual(["push left", "call at the Server"]);
    expect(cachedFromBefore(queryClient)).toBe("theirs");
  });

  test("a switch refused as not found asks again who is signed in, and reruns the access checks", async () => {
    const log: string[] = [];
    const device = { deviceUsers: [1n, 2n], refusesPush: false };
    const at = atServer(log, device);
    const { change, router, queryClient } = await openIdentityChange(
      { kind: "switched", goOn: "/next" },
      at,
      log,
    );
    await whoIsSignedIn({ queryClient, transport: at.server });
    const rerun = vi.spyOn(router, "invalidate");
    // User 2 was signed out of this Device elsewhere.
    device.deviceUsers = [1n];

    await expect(change(callNoting(log, at, notFound))).rejects.toBe(notFound);

    expect(rerun.mock.calls).toStrictEqual([[]]);
    const asked = await whoIsSignedIn({ queryClient, transport: at.server });
    expect(asked?.deviceUsers.map(({ id }) => id)).toStrictEqual([1n]);
  });
});

describe("at the Hub", () => {
  const servers = ["basement", "cabin"].map((id) =>
    create(LinkedServerSchema, { id, name: id, addresses: [`https://${id}.example`] }),
  );
  const signedOutBoth = new Set([
    "https://basement.example Bearer basement token",
    "https://cabin.example Bearer cabin token",
  ]);

  /**
   * Where a change runs at the Hub, from a Server's pages there: Ada's Hub, whose Servers only the
   * cache still lists, and Servers on which this browser keeps a session, where "basement" refuses
   * to sign its Device out and "cabin" never answers. Each sign-out a Server got is in `signedOut`.
   */
  function atHub(log: string[]) {
    const signedOut: string[] = [];
    const serverAt: ServerAt = (origin, interceptors) =>
      createRouterTransport(
        ({ service }) => {
          service(SignInService, {
            signOut: async (_request, { requestHeader }) => {
              signedOut.push(`${origin} ${requestHeader.get("Authorization")}`);
              if (origin === "https://basement.example") {
                throw new ConnectError("revoked", Code.Unauthenticated);
              }
              await new Promise(() => {});
              return {};
            },
          });
        },
        { transport: { interceptors } },
      );
    for (const { id } of servers) {
      localStorage.setItem(serverSessionKey(ada.id, id), `${id} token`);
    }
    const { hub, transport } = accountHub({ servers: [] });
    const device = { deviceUsers: [1n], refusesPush: false };
    const at = { server: serverWithPush(log, device), hub: transport, serverAt };
    return { at, hub, signedOut };
  }

  /** Opens the change at the Hub, with the Account's Servers in the cache. */
  async function openAtHub<T>(change: IdentityChange<T>, log: string[]) {
    const { at, hub, signedOut } = atHub(log);
    const opened = await openIdentityChange(change, at, log);
    const linkedServers = createConnectQueryKey({
      schema: LinkService.method.listLinkedServers,
      input: {},
      transport: at.hub,
      cardinality: "finite",
    });
    opened.queryClient.setQueryData(
      linkedServers,
      create(ListLinkedServersResponseSchema, { servers }),
    );
    return { ...opened, at, hub, signedOut };
  }

  const signInRequired = hubRefused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);

  const succeeding: {
    name: string;
    change: IdentityChange<unknown>;
    refusal?: ConnectError;
    expected: string[];
  }[] = [
    {
      name: "an Account sign-in, by any way, forgets kept sessions, then goes on in its page's place",
      change: { kind: "account signed in", goOn: "/next" },
      expected: ["call at the Hub", "cache cleared", "replaced with /next"],
    },
    {
      name: "an Account sign-out goes to sign-in, then forgets kept sessions",
      change: { kind: "account signed out", goOn: "/next" },
      expected: [
        "call at the Hub",
        "cache cleared",
        "went to /sign-in?redirect=%2Fnext (sessions kept)",
      ],
    },
    {
      name: "an Account sign-out refused for want of sign-in counts as done",
      change: { kind: "account signed out", goOn: "/next" },
      refusal: signInRequired,
      expected: [
        "call at the Hub",
        "cache cleared",
        "went to /sign-in?redirect=%2Fnext (sessions kept)",
      ],
    },
    {
      name: "deleting the Account goes to sign-in, then forgets kept sessions",
      change: { kind: "account deleted" },
      expected: ["call at the Hub", "cache cleared", "went to /sign-in (sessions kept)"],
    },
  ];

  test.each(succeeding)("$name", async ({ change, refusal, expected }) => {
    const log: string[] = [];
    const browser = browserWithPush("granted", true);
    const { change: make, at, signedOut, queryClient } = await openAtHub(change, log);

    await make(callNoting(log, at, refusal));

    // Push is never touched: the Hub has no Web Push.
    expect(log).toStrictEqual(expected);
    expect(browser.subscription.unsubscribe).not.toHaveBeenCalled();
    expect(cachedFromBefore(queryClient)).toBeUndefined();
    expect(keptSessionKeys()).toStrictEqual([]);
    // In the background, despite one refusing and one never answering.
    await waitFor(() => {
      expect(new Set(signedOut)).toStrictEqual(signedOutBoth);
    });
  });

  test("a Sign-in Link's sign-in goes where its answer says", async () => {
    const log: string[] = [];
    const { change } = await openAtHub(
      { kind: "account signed in", goOn: ({ continueTo }: { continueTo: string }) => continueTo },
      log,
    );

    // oxlint-disable-next-line require-await, typescript/require-await -- the call stands in for an RPC, and this one answers at once
    await change(async () => ({ continueTo: "/history" }));

    expect(log).toStrictEqual(["cache cleared", "replaced with /history"]);
  });

  test.each<[string, IdentityChange<unknown>, ConnectError]>([
    ["an Account sign-in", { kind: "account signed in", goOn: "/next" }, signInRequired],
    [
      "an Account sign-out refused otherwise",
      { kind: "account signed out", goOn: "/next" },
      new ConnectError("database is locked", Code.Internal),
    ],
    ["deleting the Account", { kind: "account deleted" }, signInRequired],
  ])("%s whose call fails rethrows, and keeps the sessions", async (_name, change, refusal) => {
    const log: string[] = [];
    const { change: make, at, queryClient } = await openAtHub(change, log);

    await expect(make(callNoting(log, at, refusal))).rejects.toBe(refusal);

    expect(log).toStrictEqual(["call at the Hub"]);
    expect(cachedFromBefore(queryClient)).toBe("theirs");
    expect(keptSessionKeys()).toHaveLength(2);
  });

  test("a Hub call refused for want of sign-in asks again who is signed in, and reruns the access checks", async () => {
    const log: string[] = [];
    const { onHubError, at, hub, router, queryClient } = await openAtHub(
      { kind: "account signed out", goOn: undefined },
      log,
    );
    await signedInAccount({ queryClient, transport: at.hub });
    const rerun = vi.spyOn(router, "invalidate");
    // Ada's session ended elsewhere.
    hub.signedIn = false;

    await onHubError(signInRequired);

    expect(rerun.mock.calls).toStrictEqual([[]]);
    await expect(signedInAccount({ queryClient, transport: at.hub })).resolves.toBeNull();
    // Today's: the cache and the kept sessions wait for the next sign-in (#2573).
    expect(log).toStrictEqual([]);
    expect(keptSessionKeys()).toHaveLength(2);
  });
});
