import { Code, ConnectError } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { Role, SignInCodeStatus } from "@/gen/nama/v1/sign_in_pb";

import { type ServerState, fakeServer, fill, openApp } from "./test-app";

beforeEach(() => {
  // Polling waits seconds between tries; the tests move the clock on instead.
  vi.useFakeTimers({ shouldAdvanceTime: true });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

async function getCode() {
  fireEvent.click(await screen.findByRole("button", { name: "Sign in with a code" }));
  fill("Device name", "Living room TV");
  fireEvent.click(screen.getByRole("button", { name: "Get a code" }));
}

async function nextPoll() {
  await act(() => vi.advanceTimersByTimeAsync(5000));
}

/** A signed-out Server showing "ABCD-EFGH" for "Living room TV", until `approve()` is called. */
function serverWaitingForApproval() {
  const state: ServerState = {};
  const transport = fakeServer(state, {
    startSignInCode: ({ deviceName }) => {
      expect(deviceName).toBe("Living room TV");
      return { code: "ABCD-EFGH", handle: "secret" };
    },
    pollSignInCode: ({ handle }) => {
      expect(handle).toBe("secret");
      if (state.signedIn === undefined) return { status: SignInCodeStatus.WAITING };
      return { status: SignInCodeStatus.SIGNED_IN };
    },
  });
  return {
    transport,
    approve: () => {
      state.signedIn = Role.MEMBER;
    },
  };
}

test("once the code is approved, polling signs the browser in", async () => {
  const { transport, approve } = serverWaitingForApproval();
  const router = openApp("/sign-in?redirect=%2Fsettings", transport);

  await getCode();
  expect(await screen.findByText("ABCD-EFGH")).toBeDefined();
  expect(screen.getByText(`${window.location.origin}/code`)).toBeDefined();

  await nextPoll();
  approve();
  await nextPoll();
  await screen.findByRole("heading", { name: "Settings" });
  expect(router.state.location.pathname).toBe("/settings");
});

/** A Server whose first code, "ABCD-EFGH", has expired, and whose next is "JKMN-PQRS". */
function serverWithExpiredCode() {
  const codes = ["ABCD-EFGH", "JKMN-PQRS"];
  return fakeServer(
    {},
    {
      startSignInCode: () => {
        const code = codes.shift() ?? "";
        return { code, handle: code };
      },
      pollSignInCode: ({ handle }) => ({
        status: handle === "ABCD-EFGH" ? SignInCodeStatus.EXPIRED : SignInCodeStatus.WAITING,
      }),
    },
  );
}

test("an expired code offers a new one", async () => {
  openApp("/sign-in", serverWithExpiredCode());

  await getCode();
  await nextPoll();
  fireEvent.click(await screen.findByRole("button", { name: "Get a new code" }));
  expect(await screen.findByText("JKMN-PQRS")).toBeDefined();
  expect(screen.queryByText("ABCD-EFGH")).toBeNull();
});

test("a code the Server forgot, after a restart, counts as expired", async () => {
  const transport = fakeServer(
    {},
    {
      startSignInCode: () => ({ code: "ABCD-EFGH", handle: "secret" }),
      pollSignInCode: () => {
        throw new ConnectError("no sign-in is waiting on that handle", Code.NotFound);
      },
    },
  );
  openApp("/sign-in", transport);

  await getCode();
  await nextPoll();
  expect(await screen.findByRole("button", { name: "Get a new code" })).toBeDefined();
});

test("too many waiting sign-ins says so, not that there were too many tries", async () => {
  const transport = fakeServer(
    {},
    {
      startSignInCode: () => {
        throw new ConnectError("too many sign-ins are waiting", Code.ResourceExhausted);
      },
    },
  );
  openApp("/sign-in", transport);

  await getCode();
  expect(await screen.findByText(/Too many Devices are waiting to sign in/u)).toBeDefined();
});

/** Refuses any code but the one "Living room TV" shows, as the Server would match it. */
function refuseUnlessShown(code: string) {
  if (code !== "abcdefgh") throw new ConnectError("no such code", Code.NotFound);
}

/** A signed-in Server where "Living room TV" shows `abcdefgh`, approving into `approved`. */
function serverWithCode(approved: string[] = []) {
  return fakeServer(
    { signedIn: Role.MEMBER },
    {
      lookUpSignInCode: ({ code }) => {
        refuseUnlessShown(code);
        return { deviceName: "Living room TV" };
      },
      approveSignInCode: ({ code }) => {
        refuseUnlessShown(code);
        approved.push(code);
        return {};
      },
    },
  );
}

async function lookUp(code: string) {
  fill("Sign-in Code", code);
  fireEvent.click(await screen.findByRole("button", { name: "Continue" }));
}

test("looking up a code names the Device before it's approved", async () => {
  const approved: string[] = [];
  openApp("/code", serverWithCode(approved));
  await screen.findByRole("heading", { name: "Sign in a Device" });

  await lookUp("abcdefgh");
  expect(await screen.findByText("Living room TV")).toBeDefined();
  expect(approved).toEqual([]);

  fireEvent.click(screen.getByRole("button", { name: "Sign it in" }));
  expect(await screen.findByText(/is signed in as you/u)).toBeDefined();
  expect(approved).toEqual(["abcdefgh"]);
});

test("a wrong code shows a plain error", async () => {
  openApp("/code", serverWithCode());
  await screen.findByRole("heading", { name: "Sign in a Device" });

  await lookUp("WRONG-123");
  expect(await screen.findByText(/That code doesn’t match/u)).toBeDefined();
  expect(screen.queryByRole("button", { name: "Sign it in" })).toBeNull();
});
