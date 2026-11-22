import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { keepAccountSignIn } from "@/shared/lib/account-sign-in";
import { fakeServer, newUser, openApp, type ServerState } from "@/test-app";
import { refused } from "@/test-refusal";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  sessionStorage.clear();
});

/** A signed-out Server, Linked to the Hub when `linked`, answering the rest of signing in with `signIn`. */
function linkedServer(linked: boolean, signIn: Parameters<typeof fakeServer>[1] = {}) {
  const state: ServerState = {
    serverService: { getServerInfo: () => ({ name: "Living Room", accountSignIn: linked }) },
  };
  return { state, transport: fakeServer(state, signIn) };
}

test("a Server that isn't Linked offers no Account", async () => {
  openApp("/sign-in", linkedServer(false).transport);

  await screen.findByRole("button", { name: "Sign in" });
  expect(screen.queryByRole("button", { name: "Sign in with your Account" })).toBeNull();
});

test("signing in with an Account goes to the Hub, keeping the secret that finishes it", async () => {
  // Going to another site would end the test's page, so where it went is only noted.
  const went: string[] = [];
  vi.spyOn(globalThis.location, "assign").mockImplementation((url) => {
    went.push(String(url));
  });
  const { transport } = linkedServer(true, {
    startAccountSignIn: ({ returnUrl }) => {
      expect(returnUrl).toBe(`${globalThis.location.origin}/sign-in/account?redirect=%2Fmovies`);
      return { url: "https://hub.example/server-sign-in/1", handle: "secret" };
    },
  });
  openApp("/sign-in?redirect=%2Fmovies", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign in with your Account" }));

  await expect(screen.findByRole("button", { name: "Going to the Hub…" })).resolves.toBeDefined();
  expect(went).toStrictEqual(["https://hub.example/server-sign-in/1"]);
  expect(sessionStorage.getItem("nama.account-sign-in")).toBe("secret");
});

test("while the Hub is down, signing in with an Account says why and what to do", async () => {
  const { transport } = linkedServer(true, {
    startAccountSignIn: () => {
      throw new ConnectError(
        "Signing in with an Account needs the Hub, which can't be reached right now. Sign in with your password, or try again later.",
        Code.Unavailable,
      );
    },
  });
  openApp("/sign-in", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign in with your Account" }));

  await expect(
    screen.findByText(
      /needs the Hub, which can't be reached right now\. Sign in with your password/u,
    ),
  ).resolves.toBeDefined();
  expect(screen.getByRole("button", { name: "Sign in" })).toBeDefined();
});

test("back from the Hub, the browser signs in and goes on", async () => {
  keepAccountSignIn("secret");
  const { state, transport } = linkedServer(true, {
    signInWithAccount: ({ handle }) => {
      expect(handle).toBe("secret");
      state.signedIn = newUser;
      return {};
    },
  });
  const router = openApp("/sign-in/account?redirect=%2Frequests", transport);

  await vi.waitFor(() => {
    expect(router.state.location.pathname).toBe("/requests");
  });
  expect(sessionStorage.getItem("nama.account-sign-in")).toBeNull();
});

test("a sign-in the Hub didn't approve says so, with the way back", async () => {
  const { transport } = linkedServer(true, {
    signInWithAccount: () => {
      throw refused(Code.NotFound, "not approved", {
        reason: RefusalReason.ACCOUNT_SIGN_IN_NOT_FOUND,
        field: "handle",
      });
    },
  });
  openApp("/sign-in/account", transport);

  await expect(
    screen.findByText("The Hub didn’t approve this sign-in, or it expired. Try again."),
  ).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Back to sign in" }).getAttribute("href")).toBe(
    "/sign-in",
  );
});

test("back from the Hub, “Confirm it’s you” finishes and goes back to the change", async () => {
  keepAccountSignIn("secret");
  const proofs: unknown[] = [];
  const transport = fakeServer(
    { signedIn: newUser },
    {
      reauthenticate: ({ proof }) => {
        proofs.push(proof);
        return {};
      },
    },
  );
  const router = openApp("/reauthenticate/account?redirect=%2Fcode", transport);

  await vi.waitFor(() => {
    expect(router.state.location.pathname).toBe("/code");
  });
  expect(proofs).toStrictEqual([{ case: "accountSignInHandle", value: "secret" }]);
  expect(sessionStorage.getItem("nama.account-sign-in")).toBeNull();
});

test("an approval by an Account not Linked to you says so, with the way back", async () => {
  const transport = fakeServer(
    { signedIn: newUser },
    {
      reauthenticate: () => {
        throw refused(Code.Unauthenticated, "the Account that approved isn't yours", {
          reason: RefusalReason.WRONG_ACCOUNT,
          field: "account_sign_in_handle",
        });
      },
    },
  );
  openApp("/reauthenticate/account?redirect=%2Fcode", transport);

  await expect(
    screen.findByText(
      "Another Account approved this. Sign in to the Hub with the Account Linked to you, and try again.",
    ),
  ).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Go back" }).getAttribute("href")).toBe("/code");
});
