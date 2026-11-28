import {
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import { cleanup, render, screen } from "@testing-library/react";
import { CirclePlus } from "lucide-react";
import { afterEach, expect, test } from "vite-plus/test";

import { Poster } from "@/shared/components/poster/poster";
import { posterOf } from "@/shared/lib/server-image";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

function HeatPoster() {
  return (
    <Poster
      link={{ to: "/" }}
      title="Heat"
      detail="1995"
      art={posterOf({})}
      badge={{ label: "Not on this Server", short: "Not here", Icon: CirclePlus }}
      current
    />
  );
}

const router = createRouter({
  routeTree: createRootRoute({ component: HeatPoster }),
  history: createMemoryHistory(),
});

test("a poster is a link labelled by its title and detail, marking the page it is on", async () => {
  render(<RouterProvider router={router} />, { wrapper: onServerAt() });

  const link = await screen.findByRole("link", { name: "Heat, 1995" });

  expect([link.getAttribute("aria-current"), link.textContent]).toStrictEqual([
    "page",
    "HeatNot hereNot on this Server",
  ]);
});

test("a poster's badge describes its link from inside it", async () => {
  render(<RouterProvider router={router} />, { wrapper: onServerAt() });

  const link = await screen.findByRole("link", { name: "Heat, 1995" });

  expect(link.querySelector(`[id="${link.getAttribute("aria-describedby")}"]`)?.textContent).toBe(
    "Not hereNot on this Server",
  );
});
