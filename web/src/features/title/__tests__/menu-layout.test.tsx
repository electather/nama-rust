import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test } from "vite-plus/test";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { fakeServer, newUser, openApp } from "@/test-app";
import { openMenu } from "@/test-menu";

import stylesheet from "nama-ui/styles.css?inline";

let style: HTMLStyleElement;
beforeEach(() => {
  style = document.createElement("style");
  style.textContent = stylesheet;
  document.head.append(style);
});
afterEach(() => {
  cleanup();
  style.remove();
});

/** The gutter the page's width depends on, as happy-dom lays nothing out to measure. */
function pageGutter() {
  return getComputedStyle(document.documentElement).scrollbarGutter;
}

test("opening and closing a title's More actions menu keeps the page's scrollbar gutter", async () => {
  const transport = fakeServer({ signedIn: newUser }, {}, {}, (router) => {
    router.service(BrowseService, {
      getMovie: () => ({ movie: { title: "Alien", versions: [{ id: 1n }] } }),
    });
  });
  openApp("/libraries/1/movies/tmdb-348", transport);
  await screen.findByRole("heading", { level: 1, name: "Alien" });
  const before = pageGutter();

  await openMenu("More actions");
  // Base UI's scroll lock hides the body's overflow while it holds.
  await waitFor(() => {
    expect(getComputedStyle(document.body).overflowY).toBe("hidden");
  });
  expect(pageGutter()).toBe(before);

  fireEvent.keyDown(await screen.findByRole("menu"), { key: "Escape" });
  await waitFor(() => {
    expect(getComputedStyle(document.body).overflowY).not.toBe("hidden");
  });
  expect(pageGutter()).toBe(before);
});
