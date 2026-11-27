import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { LinkedServerSchema } from "@/gen/nama/hub/v1/link_pb";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import type { UpdateServerSettingsRequest } from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, fill, newUser, openApp } from "@/test-app";
import { accountHub } from "@/test-hub";
import { plugins, summaryOf } from "@/test-plugin-list";
import { findSettingsLink, offered, pick } from "@/test-settings";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const basement = create(LinkedServerSchema, {
  id: "basement",
  name: "Basement",
  addresses: ["https://basement.example"],
});
const cabin = create(LinkedServerSchema, {
  id: "cabin",
  name: "Cabin",
  addresses: ["https://cabin.example"],
});

/**
 * A fake Server whose Account's User has `permissions`, serving the Settings `sections`, whose
 * Server URL is `url` until a change saved into `updates`.
 */
function fakeServerOf(
  permissions: Permission[],
  sections: string[],
  updates: UpdateServerSettingsRequest[] = [],
) {
  return fakeServer(
    {
      signedIn: permissions,
      serverService: {
        getServerInfo: () => ({ version: "0.3.0", settingsSections: sections }),
        getServerSettings: () => ({ settings: { url: "https://media.basement.example" } }),
        updateServerSettings: (request) => {
          updates.push(request);
          return { settings: request.settings };
        },
      },
    },
    { signInWithHubToken: () => ({ token: "session" }) },
  );
}

/** Opens Settings at the Hub, for an Account on Basement, Cabin or both, each the fake in `at`. */
function openHubSettings(
  at: Record<string, ReturnType<typeof fakeServer>>,
  url = "/?settings=account",
) {
  const servers = [basement, cabin].filter(({ addresses }) => at[addresses[0] ?? ""]);
  openApp(url, accountHub({ servers }).transport, undefined, (origin) => {
    const server = at[origin];
    if (!server) {
      throw new Error(`no fake Server at ${origin}`);
    }
    return server;
  });
}

test("the Server picker lists only the Servers where the Account's User is an Admin", async () => {
  openHubSettings({
    "https://basement.example": fakeServerOf([Permission.ADMIN], ["server"]),
    "https://cabin.example": fakeServerOf(newUser, ["server"]),
  });

  await screen.findByRole("combobox", { name: "Settings for" });

  await expect(offered("Settings for")).resolves.toStrictEqual(["Your Account", "Basement"]);
});

test("picking a Server shows its Settings, and a change applies to it", async () => {
  const updates: UpdateServerSettingsRequest[] = [];
  openHubSettings({
    "https://basement.example": fakeServerOf([Permission.ADMIN], ["server"], updates),
  });

  await screen.findByRole("combobox", { name: "Settings for" });
  pick("Settings for", "Basement");
  fireEvent.click(await findSettingsLink("Server"));
  const url = await screen.findByLabelText<HTMLInputElement>("Server URL");
  expect(url.value).toBe("https://media.basement.example");
  fill("Server URL", "https://nama.example.com");
  fireEvent.blur(url);

  await expect
    .poll(() => updates)
    .toMatchObject([{ settings: { url: "https://nama.example.com" } }]);
});

test("a Section the picked Server doesn't report says to update it, and offers nothing to do", async () => {
  openHubSettings({ "https://basement.example": fakeServerOf([Permission.ADMIN], []) });

  await screen.findByRole("combobox", { name: "Settings for" });
  pick("Settings for", "Basement");
  fireEvent.click(await findSettingsLink("Home"));

  const update = await screen.findByRole("link", { name: "Update this Server" });
  expect(update.getAttribute("href")).toBe("https://basement.example");
  expect(screen.queryByRole("button", { name: "Reset to default" })).toBeNull();
});

/** A fake Server whose Admin is signed in, serving only Plugins, which `listPlugins` answers. */
function fakeServerOfPlugins(listPlugins: ServiceImpl<typeof PluginService>["listPlugins"]) {
  return fakeServer(
    {
      signedIn: [Permission.ADMIN],
      serverService: {
        getServerInfo: () => ({ version: "0.3.0", settingsSections: ["plugins"] }),
      },
    },
    { signInWithHubToken: () => ({ token: "session" }) },
    {},
    (router) => {
      router.service(PluginService, { listPlugins });
    },
  );
}

test("a crumb that failed on one picked Server shows its name again on the next", async () => {
  let refusals = 0;
  const refused = () => {
    refusals += 1;
    throw new ConnectError("refused", Code.Internal);
  };
  const servers = {
    "https://basement.example": fakeServerOfPlugins(refused),
    "https://cabin.example": fakeServerOfPlugins(() => ({
      summaries: plugins.map((listed) => summaryOf(listed)),
    })),
  };
  // React logs the error it hands to the boundary.
  vi.spyOn(console, "error").mockReturnValue();
  openHubSettings(servers, "/?settings=plugins&plugin=tmdb&manage=basement");

  await screen.findByRole("combobox", { name: "Settings for" });
  await waitFor(() => {
    expect(refusals).toBeGreaterThan(0);
  });
  pick("Settings for", "Cabin");
  fireEvent.click(await screen.findByRole("link", { name: /TMDB/u }));

  // The header's crumbs, not the Plugin pane's own.
  await waitFor(() => {
    expect(screen.getByRole("navigation", { name: "Breadcrumbs" }).textContent).toMatch(/TMDB/u);
  });
});
