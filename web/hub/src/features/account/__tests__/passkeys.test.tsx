import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";
import { AccountSchema, AccountService, PasskeySchema } from "@hub/gen/nama/hub/v1/account_pb";
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
 * Opens `path` on a fake Hub where Ada has a passkey named "Laptop", signed in from the start
 * when `signedIn` says so. Returns the Hub's state, which a test may change.
 */
function openHub(path: string, signedIn = false) {
  let session = signedIn;
  const hub = {
    passkeys: [create(PasskeySchema, { id: "0193-laptop", name: "Laptop" })],
    /** Whether changing passkeys is refused for want of a recent sign-in. */
    stale: false,
  };
  const recentOnly = () => {
    if (hub.stale) {
      throw refused(Code.FailedPrecondition, RefusalReason.RECENT_AUTHENTICATION_REQUIRED);
    }
  };
  const transport = createRouterTransport(({ service }) => {
    service(AccountService, {
      getAccount: () => {
        if (!session) {
          throw refused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
        }
        return { account: ada, hasPassword: true };
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
        session = true;
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
    openHub("/", true);

    fireEvent.change(await screen.findByLabelText("New passkey’s name"), {
      target: { value: "Phone" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add passkey" }));

    await expect(screen.findByText("Phone")).resolves.toBeDefined();
    expect(screen.getByText("Laptop")).toBeDefined();
  });

  test("removing a passkey takes it off the list", async () => {
    openHub("/", true);

    fireEvent.click(await screen.findByRole("button", { name: "Remove Laptop" }));

    await expect(
      screen.findByText(/signs you in with your fingerprint, face or screen lock/u),
    ).resolves.toBeDefined();
  });

  test("past 10 minutes since signing in, changing passkeys offers to sign in again", async () => {
    const hub = openHub("/", true);
    hub.stale = true;

    fireEvent.click(await screen.findByRole("button", { name: "Remove Laptop" }));
    fireEvent.click(await screen.findByRole("link", { name: "Sign in again" }));

    await expect(screen.findByRole("heading", { name: "Sign in" })).resolves.toBeDefined();
    expect(hub.passkeys.map((passkey) => passkey.name)).toStrictEqual(["Laptop"]);
  });
});
