import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { accountHub, ada } from "@/test-hub";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

/** The links to Settings' sections, as its sidebar lists them. */
async function sections() {
  const nav = await screen.findByRole("navigation", { name: "Settings" });
  return within(nav)
    .getAllByRole("link")
    .map((link) => link.textContent);
}

/** The Settings dialog, once it shows. */
async function settings() {
  return within(await screen.findByRole("dialog", { name: "Settings" }));
}

test("at the Hub, the Account's page opens Settings on the Account's own Sections", async () => {
  openApp("/", accountHub().transport);

  fireEvent.click(await screen.findByRole("link", { name: "Settings" }));

  await expect(sections()).resolves.toStrictEqual(["Account", "Sign-in", "Sessions"]);
  const dialog = await settings();
  await expect(dialog.findByText(ada.email)).resolves.toBeDefined();
});

test("at a Server, the Account's Sections never show, and asking for one opens Playback", async () => {
  openSettings("account-signin", fakeServer({ signedIn: newUser }));

  const shown = await sections();

  expect(shown).not.toContain("Sessions");
  expect(shown).not.toContain("Account");
  const nav = screen.getByRole("navigation", { name: "Settings" });
  expect(within(nav).getByRole("link", { current: "page" }).textContent).toBe("Playback");
});

test("an Account without a password sets one, and can then change it", async () => {
  const { hub, transport } = accountHub({ hasPassword: false });
  openApp("/?settings=account-signin", transport);

  const dialog = await settings();
  const set = await dialog.findByRole("button", { name: "Set password" });
  fill("Password", "correct horse");
  fireEvent.click(set);

  await expect(screen.findByText("You’re signed out everywhere else.")).resolves.toBeDefined();
  await expect(screen.findByRole("button", { name: "Change password" })).resolves.toBeDefined();
  expect(hub.hasPassword).toBe(true);
});

test("a password too short is refused under its field", async () => {
  openApp("/?settings=account-signin", accountHub().transport);

  const dialog = await settings();
  await dialog.findByRole("button", { name: "Change password" });
  fill("New password", "short");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));

  await expect(screen.findByText("A password needs 8 to 1024 characters.")).resolves.toBeDefined();
});

test("past 10 minutes since signing in, Sign in again comes back to the same Section", async () => {
  const { transport } = accountHub({ stale: true });
  openApp("/?settings=account-signin", transport);

  const dialog = await settings();
  await dialog.findByRole("button", { name: "Change password" });
  fill("New password", "correct horse battery");
  fireEvent.click(screen.getByRole("button", { name: "Change password" }));
  await screen.findByText(/You signed in over 10 minutes ago/u);
  fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
  await screen.findByRole("heading", { name: "Sign in to the Hub" });
  fill("Email", ada.email);
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

  const signedInAgain = await settings();
  await expect(signedInAgain.findByLabelText("New password")).resolves.toBeDefined();
});

test("signing out another browser takes it off the list", async () => {
  const { hub, transport } = accountHub();
  openApp("/?settings=account-sessions", transport);

  const dialog = await settings();
  fireEvent.click(await dialog.findByRole("button", { name: "Sign out Safari on iPhone" }));

  await expect(
    screen.findByRole("button", { name: "Sign out everywhere else" }),
  ).resolves.toBeDefined();
  await screen.findByText("This browser");
  expect(hub.sessions.map((session) => session.name)).toStrictEqual(["Firefox on Linux"]);
});

test("signing out everywhere else leaves only this browser", async () => {
  const { hub, transport } = accountHub();
  openApp("/?settings=account-sessions", transport);

  const dialog = await settings();
  fireEvent.click(await dialog.findByRole("button", { name: "Sign out everywhere else" }));

  await waitFor(() => {
    expect(screen.queryByText("Safari on iPhone")).toBeNull();
  });
  expect(hub.sessions.map((session) => session.name)).toStrictEqual(["Firefox on Linux"]);
});

test("deleting the Account, once confirmed, signs out to the sign-in page", async () => {
  const { hub, transport } = accountHub();
  openApp("/?settings=account", transport);

  const dialog = await settings();
  fireEvent.click(await dialog.findByRole("button", { name: "Delete Account" }));
  const confirm = within(await screen.findByRole("alertdialog", { name: "Delete your Account?" }));
  fireEvent.click(confirm.getByRole("button", { name: "Delete Account" }));

  await expect(screen.findByRole("heading", { name: "Sign in to the Hub" })).resolves.toBeDefined();
  expect(hub.deleted).toBe(true);
});

test("deleting the Account past 10 minutes since signing in offers to sign in again", async () => {
  const { hub, transport } = accountHub({ stale: true });
  openApp("/?settings=account", transport);

  const dialog = await settings();
  fireEvent.click(await dialog.findByRole("button", { name: "Delete Account" }));
  const confirm = within(await screen.findByRole("alertdialog", { name: "Delete your Account?" }));
  fireEvent.click(confirm.getByRole("button", { name: "Delete Account" }));

  await expect(confirm.findByRole("button", { name: "Sign in again" })).resolves.toBeDefined();
  expect(hub.deleted).toBe(false);
});
