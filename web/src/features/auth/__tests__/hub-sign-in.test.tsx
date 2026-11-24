import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { AccountSchema, AccountService } from "@/gen/nama/hub/v1/account_pb";
import { HubService } from "@/gen/nama/hub/v1/hub_pb";
import { RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { expectUnderField } from "@/test-refusal";

afterEach(cleanup);

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

/** A Connect error as the Hub refuses with one. */
function refused(code: Code, reason: RefusalReason, params: Record<string, string> = {}) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason, params }) },
  ]);
}

function signInRequired() {
  return refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
}

/**
 * A fake Hub with Ada's Account, whose password is "correct horse", signed in from the start when
 * `signedIn` says so. It emails Sign-in Links, and its one link, "link-token", signs Ada in once.
 */
function fakeHub({ signedIn = false } = {}) {
  let session = signedIn ? ada : undefined;
  let linkUsed = false;
  const sent: { email: string; continueTo: string }[] = [];
  const transport = createRouterTransport(({ service }) => {
    service(HubService, { getHubInfo: () => ({ version: "1.0.0", signInLinks: true }) });
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw signInRequired();
        }
        return { account: session, hasPassword: true };
      },
      signIn: ({ email, password }) => {
        if (email !== ada.email || password !== "correct horse") {
          throw refused(Code.Unauthenticated, RefusalReason.WRONG_EMAIL_OR_PASSWORD);
        }
        session = ada;
        return { account: ada };
      },
      createAccount: ({ email }) => {
        if (email === ada.email) {
          throw refused(Code.AlreadyExists, RefusalReason.EMAIL_TAKEN);
        }
        session = create(AccountSchema, { id: "0193-bea", email });
        return { account: session };
      },
      sendSignInLink: ({ email, continueTo }) => {
        sent.push({ email, continueTo });
        return {};
      },
      getSignInLink: ({ token }) => {
        if (token !== "link-token" || linkUsed) {
          throw refused(Code.NotFound, RefusalReason.SIGN_IN_LINK_EXPIRED);
        }
        return { account: ada, emailProven: true };
      },
      signInWithLink: ({ token }) => {
        if (token !== "link-token" || linkUsed) {
          throw refused(Code.NotFound, RefusalReason.SIGN_IN_LINK_EXPIRED);
        }
        linkUsed = true;
        session = ada;
        return { account: ada, continueTo: "/" };
      },
      signOut: () => {
        if (!session) {
          throw signInRequired();
        }
        session = undefined;
        return {};
      },
    });
  });
  return {
    transport,
    sent,
    /** Uses the link up elsewhere, as if in another tab, without the page knowing. */
    useLinkElsewhere: () => {
      linkUsed = true;
    },
  };
}

async function signInAs(email: string, password: string) {
  await screen.findByLabelText("Email");
  fill("Email", email);
  fill("Password", password);
  fireEvent.click(await screen.findByRole("button", { name: "Sign in" }));
}

test("at the Hub, a signed-out visitor signs in to their Account with its password", async () => {
  openApp("/", fakeHub().transport);

  await signInAs(ada.email, "correct horse");

  await expect(screen.findByRole("heading", { name: "Your Account" })).resolves.toBeDefined();
  expect(screen.getByText(ada.email)).toBeDefined();
});

test("at the Hub, a wrong password is refused under the password", async () => {
  openApp("/sign-in", fakeHub().transport);

  await signInAs(ada.email, "wrong");

  await expectUnderField("Password", "That email and password don’t match an Account.");
});

test("signing the Account out goes back to sign-in", async () => {
  openApp("/", fakeHub({ signedIn: true }).transport);

  fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));

  await expect(screen.findByRole("heading", { name: "Sign in to the Hub" })).resolves.toBeDefined();
});

test("creating an Account at the Hub signs it in", async () => {
  openApp("/sign-in", fakeHub().transport);

  fireEvent.click(await screen.findByRole("link", { name: "Create an Account" }));
  await screen.findByRole("heading", { name: "Create an Account" });
  fill("Email", "bea@example.com");
  fill("Password", "a long password");
  fireEvent.click(screen.getByRole("button", { name: "Create Account" }));

  await expect(screen.findByText("bea@example.com")).resolves.toBeDefined();
});

test("an email that has an Account already is refused under the email", async () => {
  openApp("/create-account", fakeHub().transport);

  await screen.findByLabelText("Email");
  fill("Email", ada.email);
  fill("Password", "a long password");
  fireEvent.click(screen.getByRole("button", { name: "Create Account" }));

  await expectUnderField("Email", "This email has an Account already. Sign in.");
});

test("a Server has no page for creating an Account", async () => {
  openApp("/create-account", fakeServer({ signedIn: newUser }));

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});

test("a Sign-in Link is emailed, to continue where signing in was going", async () => {
  const hub = fakeHub();
  openApp("/sign-in?redirect=%2Fhistory", hub.transport);

  fireEvent.click(await screen.findByRole("link", { name: "Email me a Sign-in Link" }));
  await screen.findByRole("heading", { name: "Email a Sign-in Link" });
  fill("Email", ada.email);
  fireEvent.click(screen.getByRole("button", { name: "Email me a Sign-in Link" }));

  await screen.findByRole("heading", { name: "Check your email" });
  expect(hub.sent).toStrictEqual([{ email: ada.email, continueTo: "/history" }]);
});

test("a Sign-in Link signs its Account in", async () => {
  openApp("/sign-in-link#link-token", fakeHub().transport);

  await screen.findByText(ada.email);
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  await expect(screen.findByRole("heading", { name: "Your Account" })).resolves.toBeDefined();
});

test("an expired Sign-in Link offers a new one", async () => {
  openApp("/sign-in-link#used-token", fakeHub().transport);

  await screen.findByRole("heading", { name: "This Sign-in Link has expired" });
  expect(screen.getByRole("link", { name: "Email me a new Sign-in Link" })).toBeDefined();
});

test("a Sign-in Link used elsewhere while its page was open says it expired", async () => {
  const hub = fakeHub();
  openApp("/sign-in-link#link-token", hub.transport);
  await screen.findByText(ada.email);

  hub.useLinkElsewhere();
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  await expect(
    screen.findByRole("heading", { name: "This Sign-in Link has expired" }),
  ).resolves.toBeDefined();
});
