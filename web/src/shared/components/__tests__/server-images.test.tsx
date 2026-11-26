import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginLogo } from "@/shared/components/plugin-logo";
import { Poster } from "@/shared/components/poster/poster";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

test("a Poster loads from the Server the page talks to", async () => {
  const poster = "/media/artwork/tmdb/%2Fposter.jpg";
  const { container } = render(<Poster url={poster} title="Alien" />, { wrapper: onServerAt() });

  await screen.findByText("Alien");

  const img = container.querySelector("img");
  expect(img?.getAttribute("src")).toBe(`https://friend.example${poster}`);
  expect(img?.getAttribute("srcset")?.startsWith("https://friend.example/")).toBe(true);
});

test("a Plugin's logo loads from the Server the page talks to", () => {
  const plugin = { id: "tmdb", name: "TMDB", logoUrl: "/media/plugin-logos/tmdb/v1" };
  render(<PluginLogo plugin={plugin} />, { wrapper: onServerAt() });

  expect(screen.getByRole("img", { name: "TMDB logo" }).getAttribute("src")).toBe(
    "https://friend.example/media/plugin-logos/tmdb/v1",
  );
});
