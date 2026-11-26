import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { JobService } from "@/gen/nama/v1/job_pb";
import { admin, fakeServer, newUser, openApp } from "@/test-app";
import { openPluginSettings } from "@/test-plugins";
import { openLibraries } from "@/test-settings";

afterEach(cleanup);

/** An admin's fake Server, with one User, Alex, and one Job, Forget. */
function adminServer() {
  return fakeServer(
    { signedIn: admin },
    {},
    { listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }) },
    (router) => {
      router.service(JobService, {
        listJobs: () => ({
          jobs: [{ pluginId: "upkeep", pluginName: "Upkeep", id: "forget", name: "Forget" }],
        }),
        listJobRuns: () => ({}),
      });
    },
  );
}

/** The crumbs atop Settings' pane. */
function crumbs() {
  return within(screen.getByRole("navigation", { name: "Breadcrumbs" }));
}

/** The crumbs' current item, once it names only `title`, the list's. */
async function listCrumb(title: string) {
  await waitFor(() => {
    expect(crumbs().queryByRole("link")).toBeNull();
  });
  const current = crumbs().getByRole("listitem", { current: "page" });
  expect(current.textContent).toBe(title);
  return current;
}

/** Waits for focus to land on the list's crumb, `title`. */
async function expectFocusOnListCrumb(title: string) {
  const current = await listCrumb(title);
  await waitFor(() => {
    expect(document.activeElement).toBe(current);
  });
}

test.each([
  ["Plugins", "TMDB", () => openPluginSettings({}, "tmdb")],
  ["Libraries", "New Library", () => openLibraries(adminServer(), "new")],
  ["Jobs", "Forget", () => openApp("/code?settings=jobs&job=upkeep:forget", adminServer())],
  ["Users", "Alex", () => openApp("/code?settings=users&user=2", adminServer())],
])("%s's crumb back from %s focuses the list's crumb", async (title, opened, open) => {
  open();
  await screen.findByRole("navigation", { name: "Breadcrumbs" });
  await expect(crumbs().findByText(opened)).resolves.toBeDefined();

  fireEvent.click(crumbs().getByRole("link", { name: title }));

  await expectFocusOnListCrumb(title);
});

test("browser Back from an opened User focuses the list's crumb", async () => {
  const router = openApp("/code?settings=users", adminServer());
  fireEvent.click(await screen.findByRole("link", { name: "Alex" }));
  await expect(screen.findByRole("region", { name: "Alex" })).resolves.toBeDefined();

  router.history.back();

  await expectFocusOnListCrumb("Users");
});

test("a list opened directly leaves focus on Search", async () => {
  openApp("/code?settings=users", adminServer());
  await expect(screen.findByRole("list", { name: "Users" })).resolves.toBeDefined();

  await listCrumb("Users");
  expect(document.activeElement).toBe(screen.getByRole("combobox", { name: "Search settings" }));
});

test("the section's own link from an opened User keeps focus, as it stays in place", async () => {
  openApp("/code?settings=users&user=2", adminServer());
  await expect(screen.findByRole("region", { name: "Alex" })).resolves.toBeDefined();
  const sections = within(screen.getByRole("navigation", { name: "Settings" }));
  const users = sections.getByRole("link", { name: "Users" });
  users.focus();

  fireEvent.click(users);

  await expect(screen.findByRole("list", { name: "Users" })).resolves.toBeDefined();
  expect(document.activeElement).toBe(users);
});

test("switching section from an opened User leaves focus on the section's link", async () => {
  openApp("/code?settings=users&user=2", adminServer());
  await expect(screen.findByRole("region", { name: "Alex" })).resolves.toBeDefined();
  const devices = screen.getByRole("link", { name: "Devices" });
  devices.focus();

  fireEvent.click(devices);

  await listCrumb("Devices");
  expect(document.activeElement).toBe(devices);
});
