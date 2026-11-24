import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { HubService } from "@/gen/nama/hub/v1/hub_pb";
import { LinkedServerSchema, LinkedUserSchema, LinkService } from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { ada, hubRefused } from "@/test-hub";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const basement = create(LinkedServerSchema, { id: "0193-basement", name: "Basement" });

/**
 * A fake Hub with Ada's Account, signed in when `signedIn` says so. "SRVR-CODE" Links Basement and
 * "USER-CODE" Ada's User there; sign-in "1" is to Basement as `userName`, and every other is gone.
 * Looking a sign-in up fails the first time when `lookUpFails`, and the session ends on approving
 * when `sessionEnds`.
 */
function fakeHub({
  signedIn = true,
  userName = "Ada",
  lookUpFails = false,
  sessionEnds = false,
} = {}) {
  let session = signedIn ? ada : undefined;
  let failLookUp = lookUpFails;
  let endSession = sessionEnds;
  return createRouterTransport(({ service }) => {
    service(HubService, { getHubInfo: () => ({ version: "1.0.0", signInLinks: true }) });
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw hubRefused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { account: session, hasPassword: true };
      },
      signIn: () => {
        session = ada;
        return { account: ada };
      },
    });
    service(LinkService, {
      approveServerLink: ({ code }) => {
        if (code !== "SRVR-CODE") {
          throw hubRefused(Code.NotFound, RefusalReason.LINK_CODE_NOT_FOUND);
        }
        return { server: basement };
      },
      approveUserLink: ({ code }) => {
        if (code !== "USER-CODE") {
          throw hubRefused(Code.FailedPrecondition, RefusalReason.EMAIL_NOT_PROVEN);
        }
        return { user: create(LinkedUserSchema, { name: "Ada", serverName: "Basement" }) };
      },
      getServerSignIn: ({ id }) => {
        if (failLookUp) {
          failLookUp = false;
          throw new ConnectError("database is locked", Code.Internal);
        }
        if (id !== "1") {
          throw hubRefused(Code.NotFound, RefusalReason.SERVER_SIGN_IN_NOT_FOUND);
        }
        return { serverName: "Basement", userName };
      },
      approveServerSignIn: () => {
        if (endSession) {
          endSession = false;
          session = undefined;
          throw hubRefused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { returnUrl: "https://basement.example/sign-in/account?code=c" };
      },
    });
  });
}

/** Enters `code` in the form headed `heading`, and submits it with `action`. */
async function enterCode(heading: string, code: string, action: string) {
  const form = within(await screen.findByRole("form", { name: heading }));
  fireEvent.change(form.getByLabelText("Code"), { target: { value: code } });
  fireEvent.click(form.getByRole("button", { name: action }));
  return form;
}

test("an Account Links a Server by the code it shows", async () => {
  openApp("/", fakeHub());

  await enterCode("Link a Server", "SRVR-CODE", "Link Server");

  await expect(screen.findByText("Basement is Linked to your Account.")).resolves.toBeDefined();
});

test("a code nothing waits on is refused under the code", async () => {
  openApp("/", fakeHub());

  const form = await enterCode("Link a Server", "XXXX-XXXX", "Link Server");

  await form.findByText(/Nothing is waiting on this code/u);
  expect(form.getByLabelText("Code").getAttribute("aria-invalid")).toBe("true");
});

test("an Account Links a User by the code their Server shows them", async () => {
  openApp("/", fakeHub());

  await enterCode("Link a User", "USER-CODE", "Link User");

  await expect(
    screen.findByText("Ada on Basement is Linked to your Account."),
  ).resolves.toBeDefined();
});

test("a Link that needs the email proven offers a Sign-in Link", async () => {
  openApp("/", fakeHub());

  const form = await enterCode("Link a User", "WRONG", "Link User");

  await form.findByText(/Linking needs your email proven first/u);
  expect(form.getByRole("link", { name: "Email me a Sign-in Link" })).toBeDefined();
});

test("approving a sign-in to a Server goes back to that Server", async () => {
  // Going to another site would end the test's page, so where it went is only noted.
  const went: string[] = [];
  vi.spyOn(globalThis.location, "assign").mockImplementation((url) => {
    went.push(String(url));
  });
  openApp("/server-sign-in/1", fakeHub());

  fireEvent.click(await screen.findByRole("button", { name: "Sign in as Ada" }));

  await screen.findByRole("button", { name: "Going back to Basement…" });
  expect(went).toStrictEqual(["https://basement.example/sign-in/account?code=c"]);
});

test("a signed-out Account signs in first, then comes back to approve", async () => {
  openApp("/server-sign-in/1", fakeHub({ signedIn: false }));

  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  fill("Email", ada.email);
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  await expect(
    screen.findByRole("heading", { name: "Sign in to Basement" }),
  ).resolves.toBeDefined();
});

test("an Account whose session ended while approving signs in again, then comes back", async () => {
  openApp("/server-sign-in/1", fakeHub({ sessionEnds: true }));

  fireEvent.click(await screen.findByRole("button", { name: "Sign in as Ada" }));
  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  fill("Email", ada.email);
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  await expect(screen.findByRole("button", { name: "Sign in as Ada" })).resolves.toBeDefined();
});

test("a sign-in the Hub couldn't look up is looked up again on Try again", async () => {
  openApp("/server-sign-in/1", fakeHub({ lookUpFails: true }));

  fireEvent.click(await screen.findByRole("button", { name: "Try again" }));

  await expect(
    screen.findByRole("heading", { name: "Sign in to Basement" }),
  ).resolves.toBeDefined();
});

test("an Account with no User on the Server is told it can't sign in there", async () => {
  openApp("/server-sign-in/1", fakeHub({ userName: "" }));

  await screen.findByText(/Your Account has no User on this Server/u);
  expect(screen.queryByRole("button", { name: /Sign in as/u })).toBeNull();
});

test("a sign-in that expired says to start again on the Server", async () => {
  openApp("/server-sign-in/2", fakeHub());

  await expect(
    screen.findByText("This sign-in expired or was used. Start signing in again on your Server."),
  ).resolves.toBeDefined();
});

test("a Server has no page for approving a sign-in to a Server", async () => {
  openApp("/server-sign-in/1", fakeServer({ signedIn: newUser }));

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});
