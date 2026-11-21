import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { tmdbLogo } from "@/test-plugin-list";
import { openPluginSettings } from "@/test-plugins";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const CUSTOM_LOGO = "/media/custom-images/plugin/tmdb/logo/v2";

/** TMDB's view in Settings, whose logo is whatever `logo` says now. */
async function openTmdbWithLogo(logo: () => { logoUrl: string; customLogo: boolean }) {
  openPluginSettings(
    {
      getPlugin: () => ({
        plugin: { summary: { id: "tmdb", name: "TMDB", enabled: true, ...logo() } },
      }),
    },
    "tmdb",
  );
  const view = await screen.findByRole("region", { name: "TMDB" });
  return within(view);
}

/** Opens the menu of the pencil on the logo in `view`. */
function openLogoMenu(view: Awaited<ReturnType<typeof openTmdbWithLogo>>) {
  fireEvent.click(view.getByRole("button", { name: "Change the logo" }));
}

/** Picks `file` in the file dialog the pencil's "Upload an image" opens. */
function pickLogo(file: File) {
  const input = document.querySelector<HTMLInputElement>('input[type="file"]');
  if (!input) {
    throw new Error("the logo has no file input");
  }
  fireEvent.change(input, { target: { files: [file] } });
}

test("the pencil on a Plugin's logo uploads an image to show in its place", async () => {
  let logo = { logoUrl: tmdbLogo, customLogo: false };
  // oxlint-disable-next-line require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    logo = { logoUrl: CUSTOM_LOGO, customLogo: true };
    return new Response(null, { status: 204 });
  });
  const dialogOpened = vi.spyOn(HTMLInputElement.prototype, "click");
  const view = await openTmdbWithLogo(() => logo);
  // The pencil stands in for the Logo card the Plugin's page has.
  expect(view.queryByRole("heading", { name: "Logo" })).toBeNull();

  openLogoMenu(view);
  expect(screen.queryByRole("menuitem", { name: "Remove custom image" })).toBeNull();
  fireEvent.click(await screen.findByRole("menuitem", { name: "Upload an image" }));
  expect(dialogOpened).toHaveBeenCalledTimes(1);
  const file = new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" });
  pickLogo(file);

  await waitFor(() => {
    expect(view.getByRole("img", { name: "TMDB logo" }).getAttribute("src")).toBe(CUSTOM_LOGO);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/plugin/tmdb/logo", {
    method: "PUT",
    body: file,
  });
});

test("the pencil removes a Plugin's Custom Image, which brings back its own logo", async () => {
  let logo = { logoUrl: CUSTOM_LOGO, customLogo: true };
  // oxlint-disable-next-line require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    logo = { logoUrl: tmdbLogo, customLogo: false };
    return new Response(null, { status: 204 });
  });
  const view = await openTmdbWithLogo(() => logo);

  openLogoMenu(view);
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove custom image" }));

  await waitFor(() => {
    expect(view.getByRole("img", { name: "TMDB logo" }).getAttribute("src")).toBe(tmdbLogo);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/plugin/tmdb/logo", { method: "DELETE" });
});

test("a logo the Server refuses says what was wrong, under the Plugin's name", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("refused", { status: 415 }));
  const view = await openTmdbWithLogo(() => ({ logoUrl: tmdbLogo, customLogo: false }));

  pickLogo(new File(["text"], "logo.txt", { type: "text/plain" }));

  await expect(
    view.findByText("That isn’t a JPEG, PNG, WebP or SVG image."),
  ).resolves.toBeDefined();
});
