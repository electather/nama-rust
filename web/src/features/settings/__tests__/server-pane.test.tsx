import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type {
  UpdateImageSettingsRequest,
  UpdateServerSettingsRequest,
} from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type ServerState, fakeServer, fill, newUser } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { dropdown, openSettings, pick } from "@/test-settings";

afterEach(cleanup);

const manageSettings = [...newUser, Permission.MANAGE_SETTINGS];

function openServer(serverService: NonNullable<ServerState["serverService"]>) {
  return openSettings("server", fakeServer({ signedIn: manageSettings, serverService }));
}

test("a User who manages settings changes the Server URL, saved for everyone once its field is left", async () => {
  const updates: UpdateServerSettingsRequest[] = [];
  const images: UpdateImageSettingsRequest[] = [];
  openServer({
    getServerSettings: () => ({ settings: { url: "http://nama.lan" } }),
    updateServerSettings: (request) => {
      updates.push(request);
      return { settings: request.settings };
    },
    updateImageSettings: (request) => {
      images.push(request);
      return {};
    },
  });

  const url = await screen.findByLabelText<HTMLInputElement>("Server URL");
  expect(url.value).toBe("http://nama.lan");
  fill("Server URL", "https://nama.example.com");
  // Nothing is saved while it's typed.
  expect(updates).toStrictEqual([]);
  fireEvent.blur(url);

  await expect
    .poll(() => updates)
    .toMatchObject([{ settings: { url: "https://nama.example.com" } }]);
  // Leaving the field again saves nothing more.
  fireEvent.blur(url);
  expect(updates).toHaveLength(1);
  // Only what changed is saved.
  expect(images).toStrictEqual([]);
});

test("a Server URL that isn't a full address is said under its field once left, and isn't saved", async () => {
  const updates: UpdateServerSettingsRequest[] = [];
  openServer({
    updateServerSettings: (request) => {
      updates.push(request);
      return {};
    },
  });

  const url = await screen.findByLabelText("Server URL");
  fill("Server URL", "nama.example.com");
  fireEvent.blur(url);

  await expectUnderField("Server URL", "Enter a full address, like https://media.example.com.");
  expect(updates).toStrictEqual([]);
});

test("a Server URL the Server refuses is said under its field, and Enter tries it again", async () => {
  let tries = 0;
  openServer({
    updateServerSettings: () => {
      tries += 1;
      throw refused(Code.InvalidArgument, "the Server URL must be an http or https URL", {
        reason: RefusalReason.NOT_A_URL,
        field: "settings.url",
      });
    },
  });

  const url = await screen.findByLabelText<HTMLInputElement>("Server URL");
  fill("Server URL", "https://nama");
  fireEvent.blur(url);

  await expectUnderField("Server URL", "The Server URL must be an http or https URL.");
  // What was typed stays to be fixed.
  expect(url.value).toBe("https://nama");
  expect(tries).toBe(1);
  fireEvent.keyDown(url, { key: "Enter" });
  await expect.poll(() => tries).toBe(2);
});

test("a User who manages settings turns resizing off and gives its copies more space", async () => {
  const updates: UpdateImageSettingsRequest[] = [];
  openServer({
    getImageSettings: () => ({ settings: { resizeImages: true } }),
    updateImageSettings: (request) => {
      updates.push(request);
      return { settings: request.settings };
    },
  });

  const resize = await screen.findByRole("switch", { name: "Resize images" });
  expect(resize.getAttribute("aria-checked")).toBe("true");
  expect(dropdown("Space for resized images").textContent).toBe("1 GB");
  fireEvent.click(resize);
  pick("Space for resized images", "2 GB");

  await expect
    .poll(() => updates)
    .toMatchObject([
      { settings: { resizeImages: false, resizedCacheMib: 1024 } },
      { settings: { resizeImages: false, resizedCacheMib: 2048 } },
    ]);
});

test("a saved space that isn't one of the sizes offered stays chosen", async () => {
  openServer({
    getImageSettings: () => ({ settings: { resizeImages: true, resizedCacheMib: 512 } }),
  });

  const space = await screen.findByRole("combobox", { name: "Space for resized images" });
  expect(space.textContent).toBe("512 MB");
});

test("a User who doesn't manage settings has no Server section, and its address opens Playback", async () => {
  openSettings("server", fakeServer({ signedIn: newUser }));

  await screen.findByRole("heading", { name: "Playback" });

  const sections = within(screen.getByRole("navigation", { name: "Settings" }));
  expect(sections.queryByRole("link", { name: "Server" })).toBeNull();
  expect(sections.queryByRole("link", { name: "Plugins" })).toBeNull();
  expect(screen.queryByLabelText("Server URL")).toBeNull();
});
