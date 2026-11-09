import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { input, openPlugin, openPlugins, plugin, row } from "./test-plugins";
import { refused } from "./test-refusal";

afterEach(cleanup);

test("the Plugins list shows every Plugin with whether it is enabled, without its settings", async () => {
  openPlugins();

  expect((await row("local-auth")).getByText("Enabled")).toBeDefined();
  expect((await row("log-events")).getByText("Disabled")).toBeDefined();
  expect((await row("TMDB")).getByText("Enabled")).toBeDefined();
  expect(screen.queryByLabelText("Language")).toBeNull();
});

test("each Plugin on the list shows its name, or its id without one, and what it does", async () => {
  openPlugins();

  expect((await row("TMDB")).getByText("Finds Movies and Shows on TMDB.")).toBeDefined();
  expect((await row("local-auth")).getByText("Built-in, version 0.1.0")).toBeDefined();
});

test("a Plugin's details page shows its name, its id and what it does", async () => {
  openPlugin("tmdb");

  const page = await plugin("TMDB");
  expect(page.getByText("tmdb")).toBeDefined();
  expect(page.getByText("Finds Movies and Shows on TMDB.")).toBeDefined();
});

test("a Plugin's details page without a name is headed by its id", async () => {
  openPlugin("local-auth");

  expect((await plugin("local-auth")).getByText("Built-in, version 0.1.0")).toBeDefined();
});

test("each Plugin says whether it is Built-in or Installed", async () => {
  openPlugins();

  expect((await row("TMDB")).getByText("Built-in, version 0.1.0")).toBeDefined();
  expect((await row("test-plugin")).getByText("Installed, version 0.1.0")).toBeDefined();
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

  expect((await input("TMDB", "Language")).value).toBe("de-DE");
  expect((await plugin("TMDB")).getByText("Built-in, version 0.1.0")).toBeDefined();
  expect((await plugin("TMDB")).getByText("Enabled")).toBeDefined();
});

test("a refused Plugin's details page shows its file name and why, and no settings", async () => {
  openPlugin("out-of-range.wasm");

  const page = await plugin("out-of-range.wasm");
  expect(page.getByText("Refused")).toBeDefined();
  expect(
    page.getByText("The Server can’t run it: it wants nama-api <0.1, and this Server has 0.1.0."),
  ).toBeDefined();
  expect(page.queryByRole("heading", { name: "Settings" })).toBeNull();
  expect(page.queryByRole("button")).toBeNull();
});

test("a Plugin without settings says so on its details page", async () => {
  openPlugin("local-auth");

  expect((await plugin("local-auth")).getByText("No settings.")).toBeDefined();
});

test("the details page of a Plugin the Server doesn't have says it isn't found", async () => {
  openPlugin("no-such-plugin");

  expect(await screen.findByRole("heading", { name: "Page not found" })).toBeDefined();
});

test("disabling an enabled Plugin on its details page asks the Server to", async () => {
  const disabled: string[] = [];
  openPlugin("tmdb", {
    disablePlugin: ({ id }) => {
      disabled.push(id);
      return {};
    },
  });
  fireEvent.click((await plugin("TMDB")).getByRole("button", { name: "Disable TMDB" }));

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
  fireEvent.click((await row("log-events")).getByRole("button", { name: "Enable log-events" }));

  await waitFor(() => {
    expect(enabled).toStrictEqual(["log-events"]);
  });
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

  expect((await localAuth.findByRole("alert")).textContent).toBe(
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
        "log-events can't be enabled while radarr hands Movie Requests on",
        {
          reason: RefusalReason.FULFILLER_TAKEN,
          field: "id",
          params: { pluginId: "log-events", fulfillerPluginId: "radarr", titleKind: "Movie" },
        },
      );
    },
  });
  const logEvents = await row("log-events");
  fireEvent.click(logEvents.getByRole("button", { name: "Enable log-events" }));

  const alert = await logEvents.findByText(
    "It can’t be enabled while radarr is the Movie Fulfiller. Disable radarr first.",
  );
  expect(alert.getAttribute("role")).toBe("alert");
});

test("enabling a Plugin on its details page shows it enabled there and on the list", async () => {
  let enabled = false;
  const logEvents = () => ({ id: "log-events", version: "0.1.0", enabled });
  openPlugins({
    listPlugins: () => ({ summaries: [logEvents()] }),
    getPlugin: () => ({ plugin: logEvents() }),
    enablePlugin: () => {
      enabled = true;
      return {};
    },
  });
  fireEvent.click(await screen.findByRole("link", { name: "log-events" }));
  const page = await plugin("log-events");

  fireEvent.click(page.getByRole("button", { name: "Enable log-events" }));

  expect(await page.findByText("Enabled")).toBeDefined();
  fireEvent.click(screen.getByRole("link", { name: "Plugins" }));
  expect(await (await row("log-events")).findByText("Enabled")).toBeDefined();
});
