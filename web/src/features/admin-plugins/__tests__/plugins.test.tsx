import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { openApp } from "@/test-app";
import { tmdbLogo } from "@/test-plugin-list";
import { input, openPlugin, openPluginSettings, plugin, pluginServer, row } from "@/test-plugins";
import { refused } from "@/test-refusal";

afterEach(cleanup);

test.each([
  ["/admin/plugins", "list", { name: "Enabled" }],
  ["/admin/plugins/tmdb", "region", { name: "TMDB" }],
] as const)("the old address %s opens Settings' Plugins section", async (path, role, named) => {
  const router = openApp(path, pluginServer({}));

  await expect(screen.findByRole(role, named)).resolves.toBeDefined();
  expect(router.state.location.pathname).toBe("/");
});

test("a Plugin's view shows its name, its id and where it comes from", async () => {
  openPlugin("tmdb");

  const view = await plugin("TMDB");
  expect(view.getByText("tmdb · Built-in, version 0.1.0")).toBeDefined();
});

test("a Plugin's view without a name is headed by its id", async () => {
  openPlugin("local-auth");

  const view = await plugin("local-auth");
  expect(view.getByText("local-auth · Built-in, version 0.1.0")).toBeDefined();
});

test("each Plugin on the list says whether it is Built-in or Installed", async () => {
  openPluginSettings();

  const tmdb = await row("TMDB");
  expect(tmdb.getByText(/^Built-in, version 0\.1\.0/u)).toBeDefined();
  const testPlugin = await row("test-plugin");
  expect(testPlugin.getByText(/^Installed, version 0\.1\.0/u)).toBeDefined();
});

test("each Plugin on the list shows its logo as an image, or a generic one without", async () => {
  openPluginSettings();

  const tmdb = await row("TMDB");
  const logo = tmdb.getByRole<HTMLImageElement>("img", { name: "TMDB logo" });
  expect(logo.getAttribute("src")).toBe(tmdbLogo);
  const localAuth = await row("local-auth");
  const generic = localAuth.getByRole("img", { name: "local-auth logo" });
  expect(generic.getAttribute("src")).toBeNull();
});

test("a Plugin's view shows its logo as an image, or a generic one without", async () => {
  openPlugin("tmdb");

  const tmdb = await plugin("TMDB");
  const logo = tmdb.getByRole<HTMLImageElement>("img", { name: "TMDB logo" });
  expect(logo.getAttribute("src")).toBe(tmdbLogo);
  cleanup();
  openPlugin("local-auth");
  const localAuth = await plugin("local-auth");
  const generic = localAuth.getByRole("img", { name: "local-auth logo" });
  expect(generic.getAttribute("src")).toBeNull();
});

test("a Plugin providing Fulfillers for Movies and Shows shows both on the list and its view", async () => {
  openPluginSettings();

  const listed = await row("test-choices");
  expect(listed.getByText(/· Fulfiller for Movies · Fulfiller for Shows$/u)).toBeDefined();

  fireEvent.click(screen.getByRole("link", { name: "test-choices" }));

  const view = await plugin("test-choices");
  expect(view.getByText("Fulfiller for Movies")).toBeDefined();
  expect(view.getByText("Fulfiller for Shows")).toBeDefined();
});

test("a disabled Plugin shows the Extension Points it would take part at", async () => {
  openPluginSettings();

  const disabled = within(await screen.findByRole("list", { name: "Disabled" }));
  expect(disabled.getByText(/· Notifier · Observes Events$/u)).toBeDefined();
});

test("a refused Plugin is listed as one that can't run, with no way to enable it", async () => {
  openPluginSettings();

  fireEvent.click(await screen.findByRole("button", { name: "More for out-of-range.wasm" }));

  await screen.findByRole("menuitem", { name: "Open" });
  expect(screen.queryByRole("menuitem", { name: /^Enable/u })).toBeNull();
  const refusedRow = await row("out-of-range.wasm");
  expect(refusedRow.getByText("Can’t run")).toBeDefined();
});

test("each Plugin on the list opens its view with its settings", async () => {
  openPluginSettings();

  fireEvent.click(await screen.findByRole("link", { name: "TMDB" }));

  const language = await input("TMDB", "Language");
  expect(language.value).toBe("de-DE");
});

test("a refused Plugin's view shows its file name and why, and no settings", async () => {
  openPlugin("out-of-range.wasm");

  const view = await plugin("out-of-range.wasm");
  expect(view.getByText("Refused")).toBeDefined();
  expect(
    view.getByText(
      "The Server can’t run it: it wants Plugin contract <0.1, and this Server has 0.1.0.",
    ),
  ).toBeDefined();
  expect(view.queryByRole("heading", { name: "Settings" })).toBeNull();
  expect(view.queryByRole("button", { name: "Enable out-of-range.wasm" })).toBeNull();
});

test("a Plugin without settings has no Settings section in its view", async () => {
  openPlugin("local-auth");

  const view = await plugin("local-auth");
  expect(view.queryByRole("heading", { name: "Settings" })).toBeNull();
});

test("enabling a disabled Plugin from the list's “…” asks the Server to", async () => {
  const enabled: string[] = [];
  openPluginSettings({
    enablePlugin: ({ id }) => {
      enabled.push(id);
      return {};
    },
  });
  fireEvent.click(await screen.findByRole("button", { name: "More for test-plugin" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Enable" }));

  await waitFor(() => {
    expect(enabled).toStrictEqual(["test-plugin"]);
  });
});

test.each([
  [
    "a second Movie Fulfiller names the Plugin that is the Fulfiller",
    { reason: RefusalReason.FULFILLER_TAKEN, fulfillerPluginId: "radarr", titleKind: "Movie" },
    "It can’t be enabled while radarr is the Movie Fulfiller. Disable radarr first.",
  ],
  [
    "a Plugin that refuses to start without a reason says so plainly",
    { reason: RefusalReason.PLUGIN_REFUSED_TO_START },
    "It refused to start.",
  ],
])("enabling %s", async (_, { reason, ...params }, text) => {
  openPlugin("test-plugin", {
    enablePlugin: () => {
      throw refused(Code.FailedPrecondition, "test-plugin can't be enabled", {
        reason,
        field: "id",
        params: { pluginId: "test-plugin", ...params },
      });
    },
  });
  const view = await plugin("test-plugin");
  fireEvent.click(view.getByRole("button", { name: "Enable test-plugin" }));

  const alert = await view.findByText(text);
  expect(alert.getAttribute("role")).toBe("alert");
});

test("a homepage that isn't http or https isn't linked from a Plugin's “…”", async () => {
  openPlugin("tmdb", {
    getPlugin: () => ({
      plugin: {
        summary: { id: "tmdb", name: "TMDB", enabled: true },
        // oxlint-disable-next-line no-script-url -- the URL is the hostile input under test
        homepageUrl: "javascript:alert(1)",
      },
    }),
  });
  const view = await plugin("TMDB");

  fireEvent.click(view.getByRole("button", { name: "More for TMDB" }));

  await screen.findByRole("menuitem", { name: "Disable" });
  expect(screen.queryByRole("menuitem", { name: "Homepage" })).toBeNull();
});

test("enabling a Plugin in its view shows it enabled there and on the list", async () => {
  let enabled = false;
  const logEvents = () => ({ id: "log-events", version: "0.1.0", enabled });
  openPlugin("log-events", {
    listPlugins: () => ({ summaries: [logEvents()] }),
    getPlugin: () => ({ plugin: { summary: logEvents() } }),
    enablePlugin: () => {
      enabled = true;
      return {};
    },
  });
  const view = await plugin("log-events");

  fireEvent.click(view.getByRole("button", { name: "Enable log-events" }));

  await waitFor(() => {
    expect(view.queryByRole("button", { name: "Enable log-events" })).toBeNull();
  });
  const trail = screen.getByRole("navigation", { name: "Breadcrumbs" });
  fireEvent.click(within(trail).getByRole("link", { name: "Plugins" }));
  const listed = within(await screen.findByRole("list", { name: "Enabled" }));
  expect(listed.getByRole("link", { name: "log-events" })).toBeDefined();
});
