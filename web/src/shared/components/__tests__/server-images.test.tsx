import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { PluginLogo } from "@/shared/components/plugin-logo";
import { TileArt } from "@/shared/components/tile-art";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

test("a poster's art loads from the Server the page talks to", async () => {
  const poster = "/media/artwork/tmdb/%2Fposter.jpg";
  const look = { sizes: "150px", radius: "rounded-3xl", widths: "posterWidths" } as const;
  const { container } = render(<TileArt url={poster} thumbhash={undefined} look={look} />, {
    wrapper: onServerAt(),
  });

  const img = await waitFor(() => {
    const shown = container.querySelector("img");
    expect(shown).not.toBeNull();
    return shown;
  });
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
