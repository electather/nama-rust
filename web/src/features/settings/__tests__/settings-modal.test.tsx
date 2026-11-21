import { Code, ConnectError } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { fakeServer, newUser, openApp } from "@/test-app";
import { openSettings } from "@/test-settings";

afterEach(cleanup);

/** The links to Settings' sections, as its sidebar lists them. */
function sections() {
  const nav = screen.getByRole("navigation", { name: "Settings" });
  return within(nav)
    .getAllByRole("link")
    .map((link) => link.textContent);
}

test("the old Settings address lands on Home with Settings open on Playback", async () => {
  const router = openApp("/settings", fakeServer({ signedIn: newUser }));

  await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
  expect(router.state.location.pathname).toBe("/");
  expect(router.state.location.search).toMatchObject({ settings: "playback" });
});

test("your menu opens Settings over the page you're on, and Back closes it", async () => {
  const router = openApp("/code", fakeServer({ signedIn: newUser }));

  fireEvent.click(await screen.findByRole("button", { name: "Sam’s menu" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Settings" }));
  await expect(screen.findByRole("dialog", { name: "Settings" })).resolves.toBeDefined();
  expect(router.state.location.pathname).toBe("/code");
  router.history.back();

  await waitFor(() => {
    expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull();
  });
  expect(router.state.location.pathname).toBe("/code");
});

test("closing Settings leaves the page it was over", async () => {
  const router = openSettings("devices", fakeServer({ signedIn: newUser }));

  fireEvent.click(await screen.findByRole("button", { name: "Close Settings" }));

  await waitFor(() => {
    expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull();
  });
  expect(router.state.location.pathname).toBe("/code");
  expect(router.state.location.search.settings).toBeUndefined();
});

test("a section's link opens it, and a word that names none opens Playback", async () => {
  openApp("/code?settings=nonsense", fakeServer({ signedIn: newUser }));

  await expect(screen.findByRole("heading", { name: "Playback" })).resolves.toBeDefined();
  expect(sections()).toStrictEqual(["Sign-in", "Devices", "Playback", "Home", "Connections"]);
  fireEvent.click(screen.getByRole("link", { name: "Devices" }));

  await expect(screen.findByRole("heading", { name: "Devices" })).resolves.toBeDefined();
  expect(screen.getByRole("link", { name: "Devices" }).getAttribute("aria-current")).toBe("page");
});

test("searching narrows the sections by what they hold, and says when none matches", async () => {
  openSettings("playback", fakeServer({ signedIn: newUser }));

  const search = await screen.findByRole("searchbox", { name: "Search settings" });
  fireEvent.change(search, { target: { value: "subtitles" } });
  expect(sections()).toStrictEqual(["Playback"]);
  fireEvent.change(search, { target: { value: "wallpaper" } });

  expect(screen.getByText("No settings match “wallpaper”.")).toBeDefined();
});

function locked(): object {
  throw new ConnectError("the database is locked", Code.Internal);
}

test("a pane that doesn't load says so in place, and Try again loads it", async () => {
  let listDevices = locked;
  openSettings(
    "devices",
    fakeServer({ signedIn: newUser }, {}, { listDevices: () => listDevices() }),
  );

  await expect(screen.findByText("Couldn’t load your Devices.")).resolves.toBeDefined();
  // The rest of Settings is still there.
  expect(sections()).toContain("Playback");
  listDevices = () => ({ devices: [{ id: 1n, name: "Laptop", isCurrent: true }] });
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));

  await expect(screen.findByText("Laptop")).resolves.toBeDefined();
});
