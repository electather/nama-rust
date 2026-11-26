import {
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createContext, useContext } from "react";
import { afterEach, expect, test } from "vite-plus/test";

import { Poster } from "@/shared/components/poster/poster";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

interface Art {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
}

const ArtShown = createContext<Art>({ url: undefined, thumbhash: undefined });

function HeatPoster() {
  return <Poster link={{ to: "/" }} title="Heat" detail="1995" art={useContext(ArtShown)} />;
}

const router = createRouter({
  routeTree: createRootRoute({ component: HeatPoster }),
  history: createMemoryHistory(),
});

/** Heat's poster, showing `art`, on a page of its own. */
function heat(art: Art) {
  return (
    <ArtShown.Provider value={art}>
      <RouterProvider router={router} />
    </ArtShown.Provider>
  );
}

const first = "/media/artwork/tmdb/%2Fheat.jpg";
const thumbhash = new Uint8Array([1, 2, 3]);

/** The poster's art, its image from `url` on the Server. */
function artOf(link: HTMLElement, url: string) {
  const art = link.querySelector(`img[src="https://friend.example${url}"]`);
  if (art === null) {
    throw new Error(`no art from ${url}`);
  }
  return art;
}

/** The poster's own images, its thumbhash and its art, by `src`. */
function images(link: HTMLElement) {
  return [...link.querySelectorAll("img")].map((img) => img.getAttribute("src"));
}

test("a poster without art shows its title in its box", async () => {
  render(heat({ url: undefined, thumbhash: undefined }), { wrapper: onServerAt() });

  const link = await screen.findByRole("link", { name: "Heat, 1995" });
  expect([link.textContent, images(link)]).toStrictEqual(["Heat", []]);
});

test("a poster whose art loads shows it, not its title", async () => {
  render(heat({ url: first, thumbhash: undefined }), { wrapper: onServerAt() });

  const link = await screen.findByRole("link", { name: "Heat, 1995" });
  expect([link.textContent, images(link)]).toStrictEqual(["", [`https://friend.example${first}`]]);
});

test("a poster whose art fails to load shows its title, as one without art does", async () => {
  render(heat({ url: first, thumbhash }), { wrapper: onServerAt() });
  const link = await screen.findByRole("link", { name: "Heat, 1995" });

  fireEvent.error(artOf(link, first));

  expect([link.textContent, images(link)]).toStrictEqual(["Heat", []]);
  expect(screen.getByRole("link", { name: "Heat, 1995" })).toBe(link);
});

test("a poster whose art failed tries new art in its place", async () => {
  const { rerender } = render(heat({ url: first, thumbhash: undefined }), {
    wrapper: onServerAt(),
  });
  const link = await screen.findByRole("link", { name: "Heat, 1995" });
  fireEvent.error(artOf(link, first));
  expect(link.textContent).toBe("Heat");

  const next = "/media/artwork/tmdb/%2Fheat-2.jpg";
  rerender(heat({ url: next, thumbhash: undefined }));

  expect([link.textContent, images(link)]).toStrictEqual(["", [`https://friend.example${next}`]]);
});
