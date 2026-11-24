import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, test, vi } from "vite-plus/test";

import { fill, openApp } from "@/test-app";
import { accountHub } from "@/test-hub";

const bytes = (text: string) => new TextEncoder().encode(text).buffer;

/** What a browser's `navigator.credentials.create` gives, as far as the page reads it. */
const created = {
  rawId: bytes("credential"),
  response: {
    clientDataJSON: bytes("{}"),
    getPublicKey: () => bytes("public key"),
    getPublicKeyAlgorithm: () => -7,
  },
};

/** Has the browser create passkeys as `create` does, and returns what it answers with. */
function browserCreates(create: () => Promise<unknown>) {
  const credentials = { create };
  Object.defineProperty(navigator, "credentials", { value: credentials, configurable: true });
  return credentials;
}

/** Opens Settings' Sign-in Section at the fake Hub, once its passkeys show. */
async function openPasskeys(hub: ReturnType<typeof accountHub>) {
  openApp("/?settings=account-signin", hub.transport);
  const dialog = within(await screen.findByRole("dialog", { name: "Settings" }));
  await dialog.findByRole("list", { name: "Passkeys" });
  return dialog;
}

describe("an Account's passkeys in Settings", () => {
  beforeAll(() => {
    // happy-dom has no WebAuthn. As `Object`, every fake passes the page's `instanceof` checks.
    vi.stubGlobal("PublicKeyCredential", Object);
    vi.stubGlobal("AuthenticatorAttestationResponse", Object);
  });

  afterEach(cleanup);

  test("adding a passkey lists it by the name given", async () => {
    browserCreates(vi.fn<() => Promise<unknown>>().mockResolvedValue(created));
    const dialog = await openPasskeys(accountHub());

    fill("New passkey’s name", "Phone");
    fireEvent.click(dialog.getByRole("button", { name: "Add passkey" }));

    await expect(dialog.findByText("Phone")).resolves.toBeDefined();
    expect(dialog.getByText("Laptop")).toBeDefined();
  });

  test("a blank name is refused before the browser creates a passkey", async () => {
    const credentials = browserCreates(vi.fn<() => Promise<unknown>>().mockResolvedValue(created));
    const dialog = await openPasskeys(accountHub());

    fill("New passkey’s name", "   ");
    fireEvent.click(dialog.getByRole("button", { name: "Add passkey" }));

    await expect(
      dialog.findByText("Give it a name, such as the device it’s on."),
    ).resolves.toBeDefined();
    expect(credentials.create).not.toHaveBeenCalled();
  });

  test("cancelling the browser's prompt says nothing", async () => {
    browserCreates(
      vi
        .fn<() => Promise<unknown>>()
        .mockRejectedValue(new DOMException("cancelled", "NotAllowedError")),
    );
    const dialog = await openPasskeys(accountHub());

    fill("New passkey’s name", "Phone");
    fireEvent.click(dialog.getByRole("button", { name: "Add passkey" }));

    await expect(dialog.findByRole("button", { name: "Add passkey" })).resolves.toBeDefined();
    const said = dialog.queryAllByRole("alert").filter((alert) => alert.textContent !== "");
    expect(said).toStrictEqual([]);
  });

  test("a Hub that offers no passkeys has no form to add one, but lists those there are", async () => {
    const dialog = await openPasskeys(accountHub({ offersPasskeys: false }));

    expect(dialog.getByText("Laptop")).toBeDefined();
    expect(dialog.queryByLabelText("New passkey’s name")).toBeNull();
  });

  test("removing a passkey takes it off the list", async () => {
    const dialog = await openPasskeys(accountHub());

    fireEvent.click(dialog.getByRole("button", { name: "Remove Laptop" }));

    await expect(
      dialog.findByText(/signs you in with your fingerprint, face or screen lock/u),
    ).resolves.toBeDefined();
  });

  test("removing a passkey once this session ended elsewhere opens the sign-in page", async () => {
    const fake = accountHub();
    const dialog = await openPasskeys(fake);

    fake.hub.signedIn = false;
    fireEvent.click(dialog.getByRole("button", { name: "Remove Laptop" }));

    await expect(
      screen.findByRole("heading", { name: "Sign in to the Hub" }),
    ).resolves.toBeDefined();
    expect(fake.hub.passkeys.map((passkey) => passkey.name)).toStrictEqual(["Laptop"]);
  });
});
