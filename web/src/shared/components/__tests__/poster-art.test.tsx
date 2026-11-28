import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { PosterArt } from "@/shared/components/poster/poster-art";
import { posterOf } from "@/shared/lib/server-image";
import { posterSizes, posterTile } from "@/shared/lib/tile-shapes";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

const first = "/media/artwork/tmdb/%2Fheat.jpg";
const next = "/media/artwork/tmdb/%2Fheat-2.jpg";
const thumbhash = new Uint8Array([1, 2, 3]);
const noArt = posterOf({});

/** Heat's art at `size`. */
function heat(art: ReturnType<typeof posterOf>, size: keyof typeof posterSizes = "shelf") {
  return <PosterArt art={art} title="Heat" size={size} />;
}

/** The art's box, which `render` puts first in its container. */
function boxOf(container: HTMLElement) {
  const box = container.firstElementChild;
  if (!(box instanceof HTMLElement)) {
    throw new TypeError("no box");
  }
  return box;
}

/** The box's own images, its thumbhash and its art, by `src`. */
function images(box: HTMLElement) {
  return [...box.querySelectorAll("img")].map((img) => img.getAttribute("src"));
}

/** Fails the image the box shows from `url`, as the browser does when it can't load it. */
function failImage(box: HTMLElement, url: string) {
  const art = box.querySelector(`img[src="https://friend.example${url}"]`);
  if (art === null) {
    throw new TypeError(`no art from ${url}`);
  }
  fireEvent.error(art);
}

describe("poster art", () => {
  test("shows its title in its box without art, and renders no link", () => {
    const { container } = render(heat(noArt), { wrapper: onServerAt() });
    const box = boxOf(container);

    expect([box.textContent, images(box), container.querySelector("a")]).toStrictEqual([
      "Heat",
      [],
      null,
    ]);
  });

  test("shows an empty box at a size that isn't named", () => {
    const { container } = render(heat(noArt, "thumb"), { wrapper: onServerAt() });

    expect(boxOf(container).textContent).toBe("");
  });

  test("shows its art, not its title, once it loads", () => {
    const { container } = render(heat(posterOf({ posterUrl: first })), { wrapper: onServerAt() });
    const box = boxOf(container);

    expect([box.textContent, images(box)]).toStrictEqual(["", [`https://friend.example${first}`]]);
  });

  test("falls back on an image error, then tries new art in its place", () => {
    const { container, rerender } = render(
      heat(posterOf({ posterUrl: first, posterThumbhash: thumbhash })),
      { wrapper: onServerAt() },
    );
    const box = boxOf(container);

    failImage(box, first);
    expect([box.textContent, images(box)]).toStrictEqual(["Heat", []]);

    rerender(heat(posterOf({ posterUrl: next })));
    expect([box.textContent, images(box)]).toStrictEqual(["", [`https://friend.example${next}`]]);
  });

  test("carries the shared aspect, its size's radius and the hairline", () => {
    const { container } = render(heat(noArt, "thumb"), { wrapper: onServerAt() });
    const box = boxOf(container);

    expect([
      box.classList.contains(posterTile.aspect),
      box.classList.contains(posterSizes.thumb.radius),
      box.querySelector(`span.inset-ring-border.${posterSizes.thumb.radius}`) === null,
    ]).toStrictEqual([true, true, false]);
  });
});
