import { create } from "@bufbuild/protobuf";
import {
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vite-plus/test";

import { WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";
import { PlayProgress } from "@/shared/components/backdrop-card/play-progress";
import { Poster } from "@/shared/components/poster/poster";
import { posterOf } from "@/shared/lib/server-image";
import { tileMotion } from "@/shared/lib/tile-shapes";
import { onServerAt } from "@/test-server-at";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const router = createRouter({
  routeTree: createRootRoute({
    component: () => (
      <Poster link={{ to: "/" }} title="Heat" art={posterOf({ posterUrl: "/media/heat.jpg" })} />
    ),
  }),
  history: createMemoryHistory(),
});

/** The Poster's link, once the router has drawn it. */
async function heatLink() {
  render(<RouterProvider router={router} />, { wrapper: onServerAt() });
  const link = await screen.findByRole("link", { name: "Heat" });
  return link;
}

/** The Poster's full image, which has a `srcset` its thumbhash hasn't. */
function artOf(link: HTMLElement) {
  const art = link.querySelector("img[srcset]");
  if (art === null) {
    throw new TypeError("no art");
  }
  return art;
}

/** The names of `classes` that apply on hover, focus or press, with the variants cut off. */
function liftedBy(classes: string) {
  return classes
    .split(" ")
    .filter((name) => /(?:^|:)(?:hover|focus-visible|has-focus-visible|active):/u.test(name))
    .map((name) => name.replace(/^.*:/u, "").replace(/-?\d+$/u, ""));
}

/** A stand-in `IntersectionObserver` whose watched element comes into view when `scrollIn` runs. */
function watchOutOfView() {
  const watching = { scrollIn: () => {} };
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      public constructor(seen: (entries: { isIntersecting: boolean }[]) => void) {
        watching.scrollIn = () => {
          seen([{ isIntersecting: true }]);
        };
      }
      // oxlint-disable-next-line class-methods-use-this -- the stand-in watches nothing, so it has nothing to do
      public observe() {}
      // oxlint-disable-next-line class-methods-use-this -- the stand-in watches nothing, so it has nothing to stop
      public disconnect() {}
    },
  );
  return watching;
}

describe("a tile's motion", () => {
  test("changes only scale, shadow and stacking when hovered, focused or pressed", () => {
    const changed = liftedBy(tileMotion);

    expect(new Set(changed)).toStrictEqual(new Set(["scale", "shadow-lg", "z", "duration"]));
  });

  test("a Poster carries it, and follows the pointer by setting two style properties", async () => {
    const link = await heatLink();
    fireEvent.pointerMove(link, { clientX: 0, clientY: 0 });

    expect([
      tileMotion.split(" ").every((name) => link.classList.contains(name)),
      link.style.getPropertyValue("--mx"),
      link.style.getPropertyValue("--my"),
    ]).toStrictEqual([true, "0px", "0px"]);
  });

  test("its art is hidden until it loads, then settles in", async () => {
    const link = await heatLink();
    const art = artOf(link);
    const hidden = art.classList.contains("opacity-0");
    fireEvent.load(art);

    expect([hidden, art.classList.contains("opacity-0")]).toStrictEqual([true, false]);
  });
});

describe("a progress bar", () => {
  test("grows only once it comes into view", () => {
    const watching = watchOutOfView();
    const state = create(WatchStateSchema, { positionMs: 600_000n, durationMs: 1_200_000n });
    const { container } = render(<PlayProgress state={state} detail="10m" />);
    const bar = container.querySelector<HTMLElement>("span.rounded-full");
    const before = bar?.dataset["seen"];
    act(watching.scrollIn);

    expect([before, bar?.dataset["seen"]]).toStrictEqual([undefined, ""]);
  });
});
