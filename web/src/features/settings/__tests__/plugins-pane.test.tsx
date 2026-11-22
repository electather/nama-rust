import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { openPluginSettings } from "@/test-plugins";
import { refused } from "@/test-refusal";

afterEach(cleanup);

/** The names of the Plugins under the state `title`, in order. */
function listed(title: string) {
  return within(screen.getByRole("list", { name: title }))
    .getAllByRole("link")
    .map((link) => link.textContent);
}

test("the Plugins section lists every Plugin under its state, the ones waiting on an admin first", async () => {
  openPluginSettings();

  await screen.findByRole("list", { name: "Enabled" });
  expect(
    screen.getAllByRole("heading", { level: 4 }).map((heading) => heading.textContent),
  ).toStrictEqual(["Needs attention", "Enabled", "Disabled"]);
  expect(listed("Needs attention")).toStrictEqual(["out-of-range.wasm"]);
  expect(listed("Enabled")).toStrictEqual(["local-auth", "TMDB", "test-choices"]);
  expect(listed("Disabled")).toStrictEqual(["log-events", "test-plugin", "cleanup"]);
  expect(screen.getByText("Built-in, version 0.1.0 · Title source")).toBeDefined();
});

test("typing narrows the list, and so does where Plugins come from", async () => {
  openPluginSettings();

  const filter = await screen.findByRole("searchbox", { name: "Filter Plugins" });
  fireEvent.change(filter, { target: { value: "title source" } });
  expect(listed("Enabled")).toStrictEqual(["TMDB"]);
  expect(screen.queryByRole("list", { name: "Disabled" })).toBeNull();

  fireEvent.change(filter, { target: { value: "" } });
  fireEvent.click(screen.getByRole("radio", { name: "Installed" }));
  expect(screen.queryByRole("list", { name: "Enabled" })).toBeNull();
  expect(listed("Disabled")).toStrictEqual(["test-plugin", "cleanup"]);

  fireEvent.change(filter, { target: { value: "tmdb" } });
  expect(screen.getByText("No Plugin matches.")).toBeDefined();
});

test("a Plugin opens in place of the list, and the link over it goes back", async () => {
  const router = openPluginSettings();

  fireEvent.click(await screen.findByRole("link", { name: "TMDB" }));
  const opened = await screen.findByRole("region", { name: "TMDB" });
  expect(router.state.location.search).toMatchObject({ settings: "plugins", plugin: "tmdb" });
  expect(within(opened).getByText("Finds Movies and Shows on TMDB.")).toBeDefined();
  expect(within(opened).getByRole("heading", { name: "Settings" })).toBeDefined();
  expect(screen.queryByRole("searchbox", { name: "Filter Plugins" })).toBeNull();

  fireEvent.click(within(opened).getByRole("link", { name: "Plugins" }));
  await expect(screen.findByRole("list", { name: "Enabled" })).resolves.toBeDefined();
  expect(router.state.location.search.plugin).toBeUndefined();
});

test("a Plugin's link back to the list isn't marked as the current page", async () => {
  openPluginSettings({}, "tmdb");

  const opened = await screen.findByRole("region", { name: "TMDB" });
  expect(
    within(opened).getByRole("link", { name: "Plugins" }).getAttribute("aria-current"),
  ).toBeNull();
});

test("a Plugin's view says what each of its Extension Points means", async () => {
  openPluginSettings({}, "tmdb");

  const opened = await screen.findByRole("region", { name: "TMDB" });
  const card = within(within(opened).getByRole("region", { name: "Extension Points" }));
  expect(card.getByText("Title source")).toBeDefined();
  expect(
    card.getByText("Searches for and describes Movies and Shows, for the Libraries that ask it."),
  ).toBeDefined();
});

test("a Plugin's “…” disables it from the list, and a refusal is said in a toast", async () => {
  const asked: string[] = [];
  openPluginSettings({
    disablePlugin: ({ id }) => {
      asked.push(id);
      throw refused(Code.FailedPrecondition, "local-auth is the last authenticator", {
        reason: RefusalReason.LAST_AUTHENTICATOR,
        field: "id",
      });
    },
  });

  fireEvent.click(await screen.findByRole("button", { name: "More for local-auth" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Disable" }));

  await expect(
    screen.findByText("It can’t be disabled while no other enabled Plugin lets Users sign in."),
  ).resolves.toBeDefined();
  expect(screen.getByText("Not changed")).toBeDefined();
  expect(asked).toStrictEqual(["local-auth"]);
});

test("an enabled Plugin's view has no state: its “…” holds its links and Disable", async () => {
  const asked: string[] = [];
  openPluginSettings(
    {
      getPlugin: () => ({
        plugin: {
          summary: { id: "tmdb", name: "TMDB", enabled: true },
          homepageUrl: "https://www.themoviedb.org/",
          sourceUrl: "ftp://example.com/source",
          license: "MIT",
        },
      }),
      disablePlugin: ({ id }) => {
        asked.push(id);
        return {};
      },
    },
    "tmdb",
  );
  const opened = within(await screen.findByRole("region", { name: "TMDB" }));
  expect(opened.queryByText("Enabled")).toBeNull();
  expect(opened.queryByRole("button", { name: "Enable TMDB" })).toBeNull();
  // About keeps the licence and leaves the links to the “…”.
  expect(opened.getByText("MIT licence")).toBeDefined();
  expect(opened.queryByRole("link", { name: "Homepage" })).toBeNull();

  fireEvent.click(opened.getByRole("button", { name: "More for TMDB" }));
  const homepage = await screen.findByRole("menuitem", { name: "Homepage" });
  expect(homepage.getAttribute("href")).toBe("https://www.themoviedb.org/");
  // A link that isn't to a web page is left out.
  expect(screen.queryByRole("menuitem", { name: "Source" })).toBeNull();
  fireEvent.click(screen.getByRole("menuitem", { name: "Disable" }));

  await waitFor(() => {
    expect(asked).toStrictEqual(["tmdb"]);
  });
});

test("a disabled Plugin's view has Enable, and says why the Server refused to", async () => {
  openPluginSettings(
    {
      enablePlugin: () => {
        throw refused(Code.FailedPrecondition, "the setting url is required", {
          reason: RefusalReason.FIELD_REQUIRED,
          field: "settings.url",
        });
      },
    },
    "test-plugin",
  );
  const opened = within(await screen.findByRole("region", { name: "test-plugin" }));
  expect(opened.queryByText("Disabled")).toBeNull();
  // With no links and nothing to disable, there's no “…”.
  expect(opened.queryByRole("button", { name: "More for test-plugin" })).toBeNull();

  fireEvent.click(opened.getByRole("button", { name: "Enable test-plugin" }));

  await expect(
    opened.findByText("It needs Sonarr URL set before it can be enabled."),
  ).resolves.toBeDefined();
});

test("a Plugin to set up first is enabled from its own view, which its “…” opens", async () => {
  openPluginSettings();

  fireEvent.click(await screen.findByRole("button", { name: "More for log-events" }));
  await screen.findByRole("menuitem", { name: "Open" });
  expect(screen.queryByRole("menuitem", { name: "Enable" })).toBeNull();
  expect(screen.queryByRole("link", { name: "Set up log-events" })).toBeNull();

  fireEvent.click(screen.getByRole("menuitem", { name: "Enable…" }));
  await expect(screen.findByRole("region", { name: "log-events" })).resolves.toBeDefined();
});

test("a Plugin running without a setting it needs says so, with a button to set it up", async () => {
  openPluginSettings({
    listPlugins: () => ({
      summaries: [
        { id: "radarr", version: "0.2.0", enabled: true, settingsMissing: true },
        { id: "sonarr", version: "0.2.0", settingsMissing: true },
      ],
    }),
  });

  const waiting = within(await screen.findByRole("list", { name: "Needs attention" }));
  expect(waiting.getByText("Missing a setting")).toBeDefined();
  expect(waiting.getByRole("link", { name: "Set up radarr" }).getAttribute("href")).toContain(
    "plugin=radarr",
  );
  expect(screen.queryByRole("link", { name: "Set up sonarr" })).toBeNull();
});

test("the Plugin an address names opens with Settings, and one no Plugin has shows the list", async () => {
  openPluginSettings({}, "tmdb");
  await expect(screen.findByRole("region", { name: "TMDB" })).resolves.toBeDefined();
  cleanup();

  openPluginSettings({}, "gone");
  await expect(screen.findByRole("list", { name: "Enabled" })).resolves.toBeDefined();
});
