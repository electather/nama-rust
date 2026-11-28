import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { ShelfSkeleton } from "@/features/home/components/shelf-skeleton";
import { PosterArt } from "@/shared/components/poster/poster-art";
import { posterTile } from "@/shared/lib/tile-shapes";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

/** Whether `element` has `posterTile`'s aspect and corners, which a shelf's cards share. */
function hasPosterShape(element: Element | null) {
  return [posterTile.aspect, posterTile.radius].map((name) => element?.classList.contains(name));
}

test("a poster shelf's skeleton has the aspect and corners of the poster art that replaces it", () => {
  const { container } = render(
    <>
      <ShelfSkeleton tile={posterTile} />
      <PosterArt art={undefined} title="Heat" size="shelf" />
    </>,
    { wrapper: onServerAt() },
  );

  expect([
    hasPosterShape(container.querySelector(".animate-pulse")),
    hasPosterShape(container.lastElementChild),
  ]).toStrictEqual([
    [true, true],
    [true, true],
  ]);
});
