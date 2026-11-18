import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vite-plus/test";

import { tmdbLogo } from "@/test-plugin-list";
import { openPlugin, plugin } from "@/test-plugins";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const CUSTOM_LOGO = "/media/custom-images/plugin/tmdb/logo/v2";

/** TMDB's page, whose logo is whatever `logo` says now. */
async function openTmdbWithLogo(logo: () => { logoUrl: string; customLogo: boolean }) {
  openPlugin("tmdb", {
    getPlugin: () => ({ plugin: { summary: { id: "tmdb", name: "TMDB", ...logo() } } }),
  });
  const page = await plugin("TMDB");
  return page;
}

/** Picks `file` as the logo on `page`. */
function uploadLogo(page: Awaited<ReturnType<typeof plugin>>, file: File) {
  fireEvent.change(page.getByLabelText("Upload a Logo"), { target: { files: [file] } });
}

test("an uploaded logo goes to the Plugin's path and shows without a reload", async () => {
  let logo = { logoUrl: tmdbLogo, customLogo: false };
  // oxlint-disable-next-line require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    logo = { logoUrl: CUSTOM_LOGO, customLogo: true };
    return new Response(null, { status: 204 });
  });
  const page = await openTmdbWithLogo(() => logo);
  expect(page.queryByRole("button", { name: "Remove custom image: Logo" })).toBeNull();
  const file = new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" });

  uploadLogo(page, file);

  await waitFor(() => {
    expect(page.getByRole("img", { name: "TMDB logo" }).getAttribute("src")).toBe(CUSTOM_LOGO);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/plugin/tmdb/logo", {
    method: "PUT",
    body: file,
  });
});

test("removing a Plugin's Custom Image brings back its own logo", async () => {
  let logo = { logoUrl: CUSTOM_LOGO, customLogo: true };
  // oxlint-disable-next-line require-await -- a fake fetch answers at once
  const fetched = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
    logo = { logoUrl: tmdbLogo, customLogo: false };
    return new Response(null, { status: 204 });
  });
  const page = await openTmdbWithLogo(() => logo);

  fireEvent.click(page.getByRole("button", { name: "Remove custom image: Logo" }));

  await waitFor(() => {
    expect(page.getByRole("img", { name: "TMDB logo" }).getAttribute("src")).toBe(tmdbLogo);
  });
  expect(fetched).toHaveBeenCalledWith("/custom-images/plugin/tmdb/logo", { method: "DELETE" });
  expect(page.queryByRole("button", { name: "Remove custom image: Logo" })).toBeNull();
});

test.each([
  [413, "That file is too big: an SVG can be up to 64 KB, other images up to 20 MB."],
  [415, "That isn’t a JPEG, PNG, WebP or SVG image."],
])("a refused logo (%i) says what was wrong", async (status, message) => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("refused", { status }));
  const page = await openTmdbWithLogo(() => ({ logoUrl: tmdbLogo, customLogo: false }));

  uploadLogo(page, new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" }));

  const found1 = await page.findByRole("alert");
  expect(found1.textContent).toBe(message);
});
