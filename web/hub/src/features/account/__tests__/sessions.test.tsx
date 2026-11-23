import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService, SessionSchema } from "@hub/gen/nama/hub/v1/account_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test, vi } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    {
      desc: RefusalSchema,
      value: create(RefusalSchema, { reason, params: { recentMinutes: "10" } }),
    },
  ]);
}

/**
 * Opens the Account page on a fake Hub where Ada is signed in here, on Firefox, and also on a
 * phone and a laptop. Returns the Hub's state, which a test may change.
 */
function openAccount() {
  const hub = {
    sessions: [
      create(SessionSchema, { id: "this", name: "Firefox on Linux", isCurrent: true }),
      create(SessionSchema, { id: "phone", name: "Safari on iPhone", lastUsedAt: 1_700_000_000n }),
      create(SessionSchema, {
        id: "laptop",
        name: "Chrome on Windows",
        lastUsedAt: 1_690_000_000n,
      }),
    ],
    /** Whether signing sessions out is refused for want of a recent sign-in. */
    stale: false,
  };
  const recentOnly = () => {
    if (hub.stale) {
      throw refused(Code.FailedPrecondition, RefusalReason.RECENT_AUTHENTICATION_REQUIRED);
    }
  };
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => ({ account: ada, hasPassword: true }),
      listSessions: () => ({ sessions: hub.sessions }),
      signOutSession: ({ sessionId }) => {
        recentOnly();
        hub.sessions = hub.sessions.filter((session) => session.id !== sessionId);
        return {};
      },
      signOutOtherSessions: () => {
        recentOnly();
        hub.sessions = hub.sessions.filter((session) => session.isCurrent);
        return {};
      },
    });
  });
  globalThis.history.replaceState(null, "", "/");
  render(<RouterProvider router={getRouter(transport)} />);
  return hub;
}

async function listed() {
  const list = await screen.findByRole("list", { name: "Where you’re signed in" });
  return within(list)
    .getAllByRole("listitem")
    .map((item) => item.textContent);
}

describe("where an Account is signed in", () => {
  beforeAll(() => {
    configure({ asyncUtilTimeout: 5000 });
  });

  afterEach(() => {
    cleanup();
  });

  test("lists each session by browser, marking this one and when the others were last used", async () => {
    openAccount();

    const items = await listed();

    expect(items).toHaveLength(3);
    expect(items[0]).toContain("Firefox on Linux");
    expect(items[0]).toContain("This browser");
    expect(items[1]).toContain("Safari on iPhone");
    expect(items[1]).toContain("Last used");
    expect(screen.queryByRole("button", { name: "Sign out Firefox on Linux" })).toBeNull();
  });

  test("signing out one session takes only it off the list", async () => {
    openAccount();

    fireEvent.click(await screen.findByRole("button", { name: "Sign out Safari on iPhone" }));

    await vi.waitFor(async () => {
      await expect(listed()).resolves.toHaveLength(2);
    });
    expect(screen.queryByText("Safari on iPhone")).toBeNull();
    expect(screen.getByText("Chrome on Windows")).toBeDefined();
  });

  test("signing out everywhere else leaves only this browser", async () => {
    openAccount();

    fireEvent.click(await screen.findByRole("button", { name: "Sign out everywhere else" }));

    await vi.waitFor(async () => {
      await expect(listed()).resolves.toHaveLength(1);
    });
    expect(screen.queryByRole("button", { name: "Sign out everywhere else" })).toBeNull();
  });

  test("past 10 minutes since signing in, signing out a session offers to sign in again", async () => {
    const hub = openAccount();
    hub.stale = true;

    fireEvent.click(await screen.findByRole("button", { name: "Sign out everywhere else" }));
    fireEvent.click(await screen.findByRole("link", { name: "Sign in again" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
    expect(hub.sessions).toHaveLength(3);
  });
});
