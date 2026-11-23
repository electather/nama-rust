import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { keepAccountSignIn } from "@/features/auth/lib/account-sign-in";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
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
        "Signing in with an Account needs the Hub, which can't be reached right now. Try again later.",
        Code.Unavailable,
      );
    },
  });
  openApp("/sign-in", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign in with your Account" }));

  const said = await screen.findByText(
    /needs the Hub, which can't be reached right now\. Try again later\./u,
  );
  // A User created from an Account has no password to fall back on (ADR 0092).
  expect(said.textContent).not.toMatch(/password/iu);
  expect(screen.getByRole("button", { name: "Sign in" })).toBeDefined();
});

test("a Server opened off its reported addresses says to open it at its Server URL", async () => {
  const { transport } = linkedServer(true, {
    startAccountSignIn: () => {
      throw refused(Code.InvalidArgument, "not a Server Address", {
        reason: RefusalReason.NOT_A_SERVER_ADDRESS,
        field: "return_url",
      });
    },
  });
  openApp("/sign-in", transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign in with your Account" }));

  await expect(
    screen.findByText("Open the Server at its Server URL to sign in with an Account."),
  ).resolves.toBeDefined();
});

test("back from the Hub, the browser signs in with the Hub's code and goes on", async () => {
  keepAccountSignIn("secret");
  const { state, transport } = linkedServer(true, {
    signInWithAccount: ({ handle, code }) => {
      expect([handle, code]).toStrictEqual(["secret", "c0de"]);
      state.signedIn = newUser;
      return {};
    },
  });
  const router = openApp("/sign-in/account?redirect=%2Frequests&code=c0de", transport);

  await vi.waitFor(() => {
    expect(router.state.location.pathname).toBe("/requests");
  });
  expect(sessionStorage.getItem("nama.account-sign-in")).toBeNull();
});

test("a Server no longer Linked says who can Link it again, not to use a password", async () => {
  const { transport } = linkedServer(true, {
    signInWithAccount: () => {
      throw refused(Code.FailedPrecondition, "not linked", {
        reason: RefusalReason.NO_SERVER_LINKER,
      });
    },
  });
  openApp("/sign-in/account", transport);

  await expect(
    screen.findByText(
      "This Server isn’t Linked to the Hub any more. Ask whoever runs it to Link it again.",
    ),
  ).resolves.toBeDefined();
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
