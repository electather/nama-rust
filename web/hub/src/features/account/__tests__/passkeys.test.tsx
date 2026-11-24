import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService, PasskeySchema } from "@hub/gen/nama/hub/v1/account_pb";
import { HubService } from "@hub/gen/nama/hub/v1/hub_pb";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";
import { getRouter } from "@hub/router";
import { RouterProvider } from "@tanstack/react-router";
import { cleanup, configure, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test, vi } from "vite-plus/test";

const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });
const bytes = (text: string) => new TextEncoder().encode(text).buffer;

/** What a browser's `navigator.credentials` gives, as far as the page reads it. */
const created = {
  rawId: bytes("credential"),
  response: {
    clientDataJSON: bytes("{}"),
    getPublicKey: () => bytes("public key"),
    getPublicKeyAlgorithm: () => -7,
  },
};
const signed = {
  rawId: bytes("credential"),
  response: {
    clientDataJSON: bytes("{}"),
    authenticatorData: bytes("authenticator data"),
    signature: bytes("signature"),
    userHandle: null,
  },
};

/** Has the browser answer `create` and `get` as `credentials` does, and returns it. */
function browserCredentials<T extends object>(credentials: T) {
  Object.defineProperty(navigator, "credentials", { value: credentials, configurable: true });
  return credentials;
}

function refused(code: Code, reason: RefusalReason) {
  return new ConnectError("refused", code, undefined, [
    {
      desc: RefusalSchema,
      value: create(RefusalSchema, { reason, params: { recentMinutes: "10" } }),
    },
  ]);
}

/**
 * Opens `path` on a fake Hub with mail where Ada has a passkey named "Laptop", and a password
 * unless `password` says not, signed in from the start when `signedIn` says so. The Hub offers
 * passkeys unless `passkeys` says not. Returns the Hub's state, which a test may change.
 */
function openHub(path: string, { signedIn = false, passkeys = true, password = true } = {}) {
  const hub = {
    /** Whether this browser's session is live; another browser may end it. */
    signedIn,
    passkeys: [create(PasskeySchema, { id: "0193-laptop", name: "Laptop" })],
    /** Whether changing passkeys is refused for want of a recent sign-in. */
    stale: false,
  };
  const signedInOnly = () => {
    if (!hub.signedIn) {
      throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
    }
  };
  const recentOnly = () => {
    signedInOnly();
    if (hub.stale) {
      throw refused(Code.FailedPrecondition, RefusalReason.RECENT_AUTHENTICATION_REQUIRED);
    }
  };
  const transport = createRouterTransport(({ service }) => {
    service(HubService, { getHubInfo: () => ({ version: "0.1.0", signInLinks: true, passkeys }) });
    service(AccountService, {
      getAccount: () => {
        signedInOnly();
        return { account: ada, hasPassword: password };
      },
      listPasskeys: () => ({ passkeys: hub.passkeys }),
      startAddingPasskey: () => {
        recentOnly();
        return { challenge: new Uint8Array(32), rpId: "hub.example", userId: new Uint8Array(16) };
      },
      addPasskey: ({ name, publicKeyAlgorithm }) => {
        const passkey = create(PasskeySchema, { id: `0193-${publicKeyAlgorithm}`, name });
        hub.passkeys = [...hub.passkeys, passkey];
        return { passkey };
      },
      removePasskey: ({ passkeyId }) => {
        recentOnly();
        hub.passkeys = hub.passkeys.filter((passkey) => passkey.id !== passkeyId);
        return {};
      },
      startPasskeySignIn: () => ({ challenge: new Uint8Array(32), rpId: "hub.example" }),
      signInWithPasskey: () => {
        hub.signedIn = true;
        return { account: ada };
      },
    });
  });
  globalThis.history.replaceState(null, "", path);
  render(<RouterProvider router={getRouter(transport)} />);
  return hub;
}

describe("an Account's passkeys", () => {
  beforeAll(() => {
    configure({ asyncUtilTimeout: 5000 });
    // happy-dom has no WebAuthn. As `Object`, every fake above passes the page's `instanceof` checks.
    vi.stubGlobal("PublicKeyCredential", Object);
    vi.stubGlobal("AuthenticatorAttestationResponse", Object);
    vi.stubGlobal("AuthenticatorAssertionResponse", Object);
  });

  afterEach(() => {
    cleanup();
  });

  test("signing in with a passkey needs no email or password, and goes on to `?continue=`", async () => {
    const credentials = browserCredentials({
      get: vi.fn<() => Promise<unknown>>().mockResolvedValue(signed),
    });
    openHub("/sign-in?continue=%2Fhistory");

    fireEvent.click(await screen.findByRole("button", { name: "Sign in with a passkey" }));

    await vi.waitFor(() => {
      expect(globalThis.location.pathname).toBe("/history");
    });
    expect(credentials.get).toHaveBeenCalledWith({
      publicKey: {
        challenge: new Uint8Array(32),
        rpId: "hub.example",
        userVerification: "preferred",
      },
    });
  });

  test("a Hub that offers no passkeys has no passkey sign-in", async () => {
    browserCredentials({ get: vi.fn<() => Promise<unknown>>() });
    openHub("/sign-in", { passkeys: false });

    // Shown from the same answer from the Hub as the passkey would be.
    await expect(
      screen.findByRole("link", { name: "Email me a Sign-in Link" }),
    ).resolves.toBeDefined();
    expect(screen.queryByRole("button", { name: "Sign in with a passkey" })).toBeNull();
  });

  test("cancelling the browser's prompt says nothing", async () => {
    browserCredentials({
      get: vi
        .fn<() => Promise<unknown>>()
        .mockRejectedValue(new DOMException("cancelled", "NotAllowedError")),
    });
    openHub("/sign-in");

    fireEvent.click(await screen.findByRole("button", { name: "Sign in with a passkey" }));

    await expect(
      screen.findByRole("button", { name: "Sign in with a passkey" }),
    ).resolves.toBeDefined();
    const said = screen.queryAllByRole("alert").filter((alert) => alert.textContent !== "");
    expect(said).toStrictEqual([]);
  });

  test("adding a passkey lists it by the name given", async () => {
    browserCredentials({
      create: vi.fn<() => Promise<unknown>>().mockResolvedValue(created),
    });
    openHub("/", { signedIn: true });

    fireEvent.change(await screen.findByLabelText("New passkey’s name"), {
      target: { value: "Phone" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add passkey" }));

    await expect(screen.findByText("Phone")).resolves.toBeDefined();
    expect(screen.getByText("Laptop")).toBeDefined();
  });

  test("a Hub that offers no passkeys has no form to add one, but lists those there are", async () => {
    openHub("/", { signedIn: true, passkeys: false, password: false });

    // Shown from the same answer from the Hub as the form would be.
    await expect(screen.findByText(/as well as with Sign-in Links/u)).resolves.toBeDefined();
    await expect(screen.findByText("Laptop")).resolves.toBeDefined();
    expect(screen.queryByLabelText("New passkey’s name")).toBeNull();
  });

  test("a blank name is refused before the browser creates a passkey", async () => {
    const credentials = browserCredentials({ create: vi.fn<() => Promise<unknown>>() });
    openHub("/", { signedIn: true });

    fireEvent.change(await screen.findByLabelText("New passkey’s name"), {
      target: { value: "   " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add passkey" }));

    await expect(
      screen.findByText("Give it a name, such as the device it’s on."),
    ).resolves.toBeDefined();
    expect(credentials.create).not.toHaveBeenCalled();
  });

  test("removing a passkey takes it off the list", async () => {
    openHub("/", { signedIn: true });

    fireEvent.click(await screen.findByRole("button", { name: "Remove Laptop" }));

    await expect(
      screen.findByText(/signs you in with your fingerprint, face or screen lock/u),
    ).resolves.toBeDefined();
  });

  test("past 10 minutes since signing in, changing passkeys offers to sign in again", async () => {
    const hub = openHub("/", { signedIn: true });
    hub.stale = true;

    fireEvent.click(await screen.findByRole("button", { name: "Remove Laptop" }));
    fireEvent.click(await screen.findByRole("link", { name: "Sign in again" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
    expect(hub.passkeys.map((passkey) => passkey.name)).toStrictEqual(["Laptop"]);
  });

  test("adding a passkey once this session ended elsewhere opens the sign-in page", async () => {
    const credentials = browserCredentials({ create: vi.fn<() => Promise<unknown>>() });
    const hub = openHub("/", { signedIn: true });

    fireEvent.change(await screen.findByLabelText("New passkey’s name"), {
      target: { value: "Phone" },
    });
    hub.signedIn = false;
    fireEvent.click(screen.getByRole("button", { name: "Add passkey" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
    expect(credentials.create).not.toHaveBeenCalled();
  });

  test("removing a passkey once this session ended elsewhere opens the sign-in page", async () => {
    const hub = openHub("/", { signedIn: true });

    const remove = await screen.findByRole("button", { name: "Remove Laptop" });
    hub.signedIn = false;
    fireEvent.click(remove);

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
    expect(hub.passkeys.map((passkey) => passkey.name)).toStrictEqual(["Laptop"]);
  });
});
