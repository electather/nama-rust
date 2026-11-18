import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { tmdbLogo } from "@/test-plugin-list";
import { input, openPlugin, openPlugins, plugin, row } from "@/test-plugins";
import { refused } from "@/test-refusal";

afterEach(cleanup);

test("the Plugins list shows every Plugin with whether it is enabled, without its settings", async () => {
  openPlugins();

  const found1 = await row("local-auth");
  expect(found1.getByText("Enabled")).toBeDefined();
  const found2 = await row("log-events");
  expect(found2.getByText("Disabled")).toBeDefined();
  const found3 = await row("TMDB");
  expect(found3.getByText("Enabled")).toBeDefined();
  expect(screen.queryByLabelText("Language")).toBeNull();
});

test("each Plugin on the list shows its name, or its id without one, and what it does", async () => {
  openPlugins();

  const found4 = await row("TMDB");
  expect(found4.getByText("Finds Movies and Shows on TMDB.")).toBeDefined();
  const found5 = await row("local-auth");
  expect(found5.getByText("Built-in, version 0.1.0")).toBeDefined();
});

test("a Plugin's details page shows its name, its id and what it does", async () => {
  openPlugin("tmdb");

  const page = await plugin("TMDB");
  expect(page.getByText("tmdb")).toBeDefined();
  expect(page.getByText("Finds Movies and Shows on TMDB.")).toBeDefined();
});

test("a Plugin's details page without a name is headed by its id", async () => {
  openPlugin("local-auth");

  const found6 = await plugin("local-auth");
  expect(found6.getByText("Built-in, version 0.1.0")).toBeDefined();
});

test("each Plugin says whether it is Built-in or Installed", async () => {
  openPlugins();

  const found7 = await row("TMDB");
  expect(found7.getByText("Built-in, version 0.1.0")).toBeDefined();
  const found8 = await row("test-plugin");
  expect(found8.getByText("Installed, version 0.1.0")).toBeDefined();
});

test("each Plugin on the list shows its logo as an image, or a generic one without", async () => {
  openPlugins();

  const found101 = await row("TMDB");
  const logo = found101.getByRole<HTMLImageElement>("img", { name: "TMDB logo" });
  expect(logo.getAttribute("src")).toBe(tmdbLogo);
  const found102 = await row("local-auth");
  const generic = found102.getByRole("img", { name: "local-auth logo" });
  expect(generic.getAttribute("src")).toBeNull();
});

test("a Plugin's details page shows its logo as an image, or a generic one without", async () => {
  openPlugin("tmdb");

  const found103 = await plugin("TMDB");
  const logo = found103.getByRole<HTMLImageElement>("img", { name: "TMDB logo" });
  expect(logo.getAttribute("src")).toBe(tmdbLogo);
  cleanup();
  openPlugin("local-auth");
  const found104 = await plugin("local-auth");
  const generic = found104.getByRole("img", { name: "local-auth logo" });
  expect(generic.getAttribute("src")).toBeNull();
});

test("a Plugin providing Fulfillers for Movies and Shows shows both on the list and its page", async () => {
  openPlugins();

  const listed = await row("test-choices");
  expect(listed.getByText("Fulfiller for Movies")).toBeDefined();
  expect(listed.getByText("Fulfiller for Shows")).toBeDefined();
  expect(listed.queryByText("Notifier")).toBeNull();

  fireEvent.click(screen.getByRole("link", { name: "test-choices" }));

  const page = await plugin("test-choices");
  expect(page.getByText("Fulfiller for Movies")).toBeDefined();
  expect(page.getByText("Fulfiller for Shows")).toBeDefined();
});

test("a disabled Plugin shows the Extension Points it would take part at", async () => {
  openPlugins();

  const listed = await row("test-plugin");
  expect(listed.getByText("Disabled")).toBeDefined();
  expect(listed.getByText("Notifier")).toBeDefined();
  expect(listed.getByText("Observes Events")).toBeDefined();
});

test("a refused Plugin is listed as Refused, with no way to enable it", async () => {
  openPlugins();

  const refusedRow = await row("out-of-range.wasm");
  expect(refusedRow.getByText("Refused")).toBeDefined();
  expect(refusedRow.queryByRole("button")).toBeNull();
});

test("each Plugin on the list opens its details page with its settings", async () => {
  openPlugins();

  fireEvent.click(await screen.findByRole("link", { name: "TMDB" }));

  const found9 = await input("TMDB", "Language");
  expect(found9.value).toBe("de-DE");
  const found10 = await plugin("TMDB");
  expect(found10.getByText("Built-in, version 0.1.0")).toBeDefined();
  const found11 = await plugin("TMDB");
  expect(found11.getByText("Enabled")).toBeDefined();
});

test("a refused Plugin's details page shows its file name and why, and no settings", async () => {
  openPlugin("out-of-range.wasm");

  const page = await plugin("out-of-range.wasm");
  expect(page.getByText("Refused")).toBeDefined();
  expect(
    page.getByText(
      "The Server can’t run it: it wants Plugin contract <0.1, and this Server has 0.1.0.",
    ),
  ).toBeDefined();
  expect(page.queryByRole("heading", { name: "Settings" })).toBeNull();
  expect(page.queryByRole("button")).toBeNull();
});

test("a Plugin without settings says so on its details page", async () => {
  openPlugin("local-auth");

  const found12 = await plugin("local-auth");
  expect(found12.getByText("No settings.")).toBeDefined();
});

test("the details page of a Plugin the Server doesn't have says it isn't found", async () => {
  openPlugin("no-such-plugin");

  await expect(screen.findByRole("heading", { name: "Page not found" })).resolves.toBeDefined();
});

test("disabling an enabled Plugin on its details page asks the Server to", async () => {
  const disabled: string[] = [];
  openPlugin("tmdb", {
    disablePlugin: ({ id }) => {
      disabled.push(id);
      return {};
    },
  });
  const found13 = await plugin("TMDB");
  fireEvent.click(found13.getByRole("button", { name: "Disable TMDB" }));

  await waitFor(() => {
    expect(disabled).toStrictEqual(["tmdb"]);
  });
});

test("enabling a disabled Plugin on the list asks the Server to", async () => {
  const enabled: string[] = [];
  openPlugins({
    enablePlugin: ({ id }) => {
      enabled.push(id);
      return {};
    },
  });
  const found14 = await row("test-plugin");
  fireEvent.click(found14.getByRole("button", { name: "Enable test-plugin" }));

  await waitFor(() => {
    expect(enabled).toStrictEqual(["test-plugin"]);
  });
});

test("a disabled Plugin missing a required setting offers Set up, which opens its details page", async () => {
  openPlugins();

  const logEvents = await row("log-events");
  expect(logEvents.queryByRole("button", { name: "Enable log-events" })).toBeNull();
  fireEvent.click(logEvents.getByRole("link", { name: "Set up log-events" }));

  const found15 = await input("log-events", "Path");
  expect(found15.required).toBe(false);
});

test("an enabled Plugin missing a required setting is marked as needing attention", async () => {
  openPlugins({
    listPlugins: () => ({
      summaries: [{ id: "radarr", version: "0.2.0", enabled: true, settingsMissing: true }],
    }),
  });

  const radarr = await row("radarr");
  expect(radarr.getByText("Needs attention")).toBeDefined();
  expect(radarr.getByRole("button", { name: "Disable radarr" })).toBeDefined();
});

test("disabling the only Plugin Users sign in with says why it can’t be", async () => {
  openPlugins({
    disablePlugin: () => {
      throw refused(
        Code.FailedPrecondition,
        "local-auth can't be disabled while no other enabled Plugin lets Users sign in",
        {
          reason: RefusalReason.LAST_AUTHENTICATOR,
          field: "id",
        },
      );
    },
  });
  const localAuth = await row("local-auth");
  fireEvent.click(localAuth.getByRole("button", { name: "Disable local-auth" }));

  const found16 = await localAuth.findByRole("alert");
  expect(found16.textContent).toBe(
    "It can’t be disabled while no other enabled Plugin lets Users sign in.",
  );
});

test("enabling a Plugin missing a required setting names the setting", async () => {
  openPlugin("log-events", {
    enablePlugin: () => {
      throw refused(Code.InvalidArgument, "the setting path is required", {
        reason: RefusalReason.FIELD_REQUIRED,
        field: "settings.path",
      });
    },
  });
  const logEvents = await plugin("log-events");
  fireEvent.click(logEvents.getByRole("button", { name: "Enable log-events" }));

  // Its settings form has an alert under each input too, empty until a save is refused.
  const alert = await logEvents.findByText("It needs Path set before it can be enabled.");
  expect(alert.getAttribute("role")).toBe("alert");
});

test("enabling a second Movie Fulfiller names the Plugin that is the Fulfiller", async () => {
  openPlugins({
    enablePlugin: () => {
      throw refused(
        Code.FailedPrecondition,
        "test-plugin can't be enabled while radarr hands Movie Requests on",
        {
          reason: RefusalReason.FULFILLER_TAKEN,
          field: "id",
          params: { pluginId: "test-plugin", fulfillerPluginId: "radarr", titleKind: "Movie" },
        },
      );
    },
  });
  const testPlugin = await row("test-plugin");
  fireEvent.click(testPlugin.getByRole("button", { name: "Enable test-plugin" }));

  const alert = await testPlugin.findByText(
    "It can’t be enabled while radarr is the Movie Fulfiller. Disable radarr first.",
  );
  expect(alert.getAttribute("role")).toBe("alert");
});

test.each([
  [
    "says why in its words",
    { reason: "the key was turned down" },
    "It refused to start: the key was turned down.",
  ],
  ["without a reason says so plainly", {}, "It refused to start."],
])("enabling a Plugin that refuses to start %s", async (_, params, text) => {
  openPlugins({
    enablePlugin: () => {
      throw refused(Code.FailedPrecondition, "test-plugin refused to start", {
        reason: RefusalReason.PLUGIN_REFUSED_TO_START,
        field: "id",
        params: { pluginId: "test-plugin", ...params },
      });
    },
  });
  const testPlugin = await row("test-plugin");
  fireEvent.click(testPlugin.getByRole("button", { name: "Enable test-plugin" }));

  const alert = await testPlugin.findByText(text);
  expect(alert.getAttribute("role")).toBe("alert");
});

test("enabling a Plugin on its details page shows it enabled there and on the list", async () => {
  let enabled = false;
  const logEvents = () => ({ id: "log-events", version: "0.1.0", enabled });
  openPlugins({
    listPlugins: () => ({ summaries: [logEvents()] }),
    getPlugin: () => ({ plugin: { summary: logEvents() } }),
    enablePlugin: () => {
      enabled = true;
      return {};
    },
  });
  fireEvent.click(await screen.findByRole("link", { name: "log-events" }));
  const page = await plugin("log-events");

  fireEvent.click(page.getByRole("button", { name: "Enable log-events" }));

  await expect(page.findByText("Enabled")).resolves.toBeDefined();
  fireEvent.click(screen.getByRole("link", { name: "Plugins" }));
  const found17 = await row("log-events");
  await expect(found17.findByText("Enabled")).resolves.toBeDefined();
});
