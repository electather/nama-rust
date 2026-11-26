import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterAll, afterEach, beforeAll, describe, expect, test, vi } from "vite-plus/test";

import { AccountSchema, AccountService } from "@/gen/nama/hub/v1/account_pb";
import { HubService } from "@/gen/nama/hub/v1/hub_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { ada, hubRefused } from "@/test-hub";
import { expectUnderField } from "@/test-refusal";

afterEach(cleanup);

function signInRequired() {
  return hubRefused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
}

const bytes = (text: string) => new TextEncoder().encode(text).buffer;

/** What a browser's `navigator.credentials.get` gives for Ada's passkey, as far as the page reads it. */
const adasPasskey = {
  rawId: bytes("ada-passkey"),
  response: {
    clientDataJSON: bytes("{}"),
    authenticatorData: bytes("authenticator data"),
    signature: bytes("signature"),
    userHandle: bytes("0193-ada"),
  },
};

/** Has the browser sign in with passkeys as `get` does. */
function browserGets(get: () => Promise<unknown>) {
  Object.defineProperty(navigator, "credentials", { value: { get }, configurable: true });
}

/**
 * A fake Hub with Ada's Account, whose password is "correct horse", signed in from the start when
 * `signedIn` says so. It emails Sign-in Links, and its one link, "link-token", signs Ada in once;
 * it fails to look up "broken-token". Where it offers `passkeys`, it accepts only Ada's.
 */
function fakeHub({ signedIn = false, passkeys = false } = {}) {
  let session = signedIn ? ada : undefined;
  let linkUsed = false;
  const sent: { email: string; continueTo: string }[] = [];
  const transport = createRouterTransport(({ service }) => {
    service(HubService, { getHubInfo: () => ({ version: "1.0.0", signInLinks: true, passkeys }) });
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw signInRequired();
        }
        return { account: session, hasPassword: true };
      },
      signIn: ({ email, password }) => {
        if (email !== ada.email || password !== "correct horse") {
          throw hubRefused(Code.Unauthenticated, RefusalReason.WRONG_EMAIL_OR_PASSWORD);
        }
        session = ada;
        return { account: ada };
      },
      createAccount: ({ email }) => {
        if (email === ada.email) {
          throw hubRefused(Code.AlreadyExists, RefusalReason.EMAIL_TAKEN);
        }
        session = create(AccountSchema, { id: "0193-bea", email });
        return { account: session };
      },
      sendSignInLink: ({ email, continueTo }) => {
        sent.push({ email, continueTo });
        return {};
      },
      getSignInLink: ({ token }) => {
        if (token === "broken-token") {
          throw new ConnectError("database is locked", Code.Internal);
        }
        if (token !== "link-token" || linkUsed) {
          throw hubRefused(Code.NotFound, RefusalReason.SIGN_IN_LINK_EXPIRED);
        }
        return { account: ada, emailProven: true };
      },
      signInWithLink: ({ token }) => {
        if (token !== "link-token" || linkUsed) {
          throw hubRefused(Code.NotFound, RefusalReason.SIGN_IN_LINK_EXPIRED);
        }
        linkUsed = true;
        session = ada;
        return { account: ada, continueTo: "/" };
      },
      startPasskeySignIn: () => ({ challenge: new Uint8Array(32), rpId: "hub.example" }),
      signInWithPasskey: ({ credentialId }) => {
        if (new TextDecoder().decode(credentialId) !== "ada-passkey") {
          throw hubRefused(Code.Unauthenticated, RefusalReason.PASSKEY_NOT_ACCEPTED);
        }
        session = ada;
        return { account: ada };
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

test("creating an Account goes on to where signing in was going", async () => {
  const router = openApp("/sign-in?redirect=%2Fcode", fakeHub().transport);

  fireEvent.click(await screen.findByRole("link", { name: "Create an Account" }));
  await screen.findByRole("heading", { name: "Create an Account" });
  fill("Email", "bea@example.com");
  fill("Password", "a long password");
  fireEvent.click(screen.getByRole("button", { name: "Create Account" }));

  await screen.findByRole("heading", { name: "Sign in a Device" });
  expect(router.state.location.pathname).toBe("/code");
});

test("signing in instead of creating an Account keeps where signing in was going", async () => {
  const router = openApp("/create-account?redirect=%2Fcode", fakeHub().transport);

  fireEvent.click(await screen.findByRole("link", { name: "Sign in" }));

  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  expect(router.state.location.href).toBe("/sign-in?redirect=%2Fcode");
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

// The fake Hub doesn't answer `nama.v1`, so a page that called it would fail instead.
test.each(["/sign-in/code", "/sign-in/code/waiting", "/sign-in/account", "/invite/a-token"])(
  "the Hub has no Server's page %s",
  async (path) => {
    openApp(path, fakeHub().transport);

    await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
  },
);

test("a Server opens its page for signing in with a code", async () => {
  openApp("/sign-in/code", fakeServer({}));

  await expect(screen.findByLabelText("Device name")).resolves.toBeDefined();
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

test("a Sign-in Link the Hub can't look up says so, and offers to try again", async () => {
  openApp("/sign-in-link#broken-token", fakeHub().transport);

  await screen.findByText("The Hub couldn’t open this Sign-in Link just now.");
  expect(screen.getByRole("button", { name: "Try again" })).toBeDefined();
});

test("a browser without passkeys has no passkey sign-in", async () => {
  openApp("/sign-in", fakeHub({ passkeys: true }).transport);

  await screen.findByRole("link", { name: "Email me a Sign-in Link" });
  expect(screen.queryByRole("button", { name: "Sign in with a passkey" })).toBeNull();
});

describe("signing in at the Hub with a passkey", () => {
  beforeAll(() => {
    // happy-dom has no WebAuthn. As `Object`, every fake passes the page's `instanceof` checks.
    vi.stubGlobal("PublicKeyCredential", Object);
    vi.stubGlobal("AuthenticatorAssertionResponse", Object);
  });

  afterAll(() => {
    vi.unstubAllGlobals();
  });

  test("a Hub that offers no passkeys has no passkey sign-in", async () => {
    openApp("/sign-in", fakeHub().transport);

    await screen.findByRole("link", { name: "Email me a Sign-in Link" });
    expect(screen.queryByRole("button", { name: "Sign in with a passkey" })).toBeNull();
  });

  test("a passkey signs in with no email or password, and goes on to the redirect", async () => {
    browserGets(vi.fn<() => Promise<unknown>>().mockResolvedValue(adasPasskey));
    const router = openApp("/sign-in?redirect=%2Fcode", fakeHub({ passkeys: true }).transport);

    fireEvent.click(await screen.findByRole("button", { name: "Sign in with a passkey" }));

    await screen.findByRole("heading", { name: "Sign in a Device" });
    expect(router.state.location.pathname).toBe("/code");
  });

  test("while the browser's passkey prompt is open, the button says it's signing in", async () => {
    browserGets(async () => {
      await new Promise<never>(() => {});
    });
    openApp("/sign-in", fakeHub({ passkeys: true }).transport);

    fireEvent.click(await screen.findByRole("button", { name: "Sign in with a passkey" }));

    await expect(screen.findByRole("button", { name: "Signing in…" })).resolves.toHaveProperty(
      "disabled",
      true,
    );
  });

  test("a passkey the Hub refuses says so", async () => {
    browserGets(
      vi.fn<() => Promise<unknown>>().mockResolvedValue({ ...adasPasskey, rawId: bytes("other") }),
    );
    openApp("/sign-in", fakeHub({ passkeys: true }).transport);

    fireEvent.click(await screen.findByRole("button", { name: "Sign in with a passkey" }));

    await expect(screen.findByText("The Hub didn’t accept that passkey.")).resolves.toBeDefined();
  });

  test("cancelling the browser's passkey prompt says nothing", async () => {
    browserGets(
      vi
        .fn<() => Promise<unknown>>()
        .mockRejectedValue(new DOMException("cancelled", "NotAllowedError")),
    );
    openApp("/sign-in", fakeHub({ passkeys: true }).transport);

    const button = await screen.findByRole("button", { name: "Sign in with a passkey" });
    fireEvent.click(button);

    await waitFor(() => {
      expect(button).toHaveProperty("disabled", false);
    });
    const said = screen.queryAllByRole("alert").filter((alert) => alert.textContent !== "");
    expect(said).toStrictEqual([]);
  });
});
