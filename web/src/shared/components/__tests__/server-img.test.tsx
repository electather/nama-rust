import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { ServerImg } from "@/shared/components/server-img";
import {
  type ServerImage,
  backdropOf,
  pluginLogoOf,
  posterOf,
  stillOf,
  thumbnailOf,
} from "@/shared/lib/server-image";
import { openApp } from "@/test-app";
import { fakeServer, newUser } from "@/test-server";
import { onServerAt } from "@/test-server-at";

afterEach(cleanup);

const friend = "https://friend.example";
const poster = posterOf({ posterUrl: "/media/poster.jpg", posterThumbhash: new Uint8Array([1]) });
const backdrop = backdropOf({ backdropUrl: "/media/backdrop.jpg" });

/** `images` shown on the friend's Server, holding `info`, with "Heat" as the fallback. */
function show(images: (ServerImage | undefined)[], info?: Parameters<typeof onServerAt>[1]) {
  return render(
    <ServerImg images={images} sizes="150px" className="size-full" alt="Art" fallback="Heat" />,
    { wrapper: onServerAt(friend, info) },
  );
}

/** The image shown, not its thumbhash, by `src` and `srcset`. */
async function shownImage() {
  const img = await screen.findByRole("img", { name: "Art" });
  return { src: img.getAttribute("src"), srcSet: img.getAttribute("srcset") };
}

describe("an image the Server sent", () => {
  test("on a page offers the widths the page's Server lists for its kind", async () => {
    const posterUrl = "/media/artwork/tmdb/%2Falien.jpg";
    const { transport } = fakeServer(
      {
        signedIn: newUser,
        libraries: [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }],
        info: { imageWidths: [320, 480, 640], posterWidths: [320, 480] },
      },
      { BrowseService: { listMovies: () => ({ movies: [{ title: "Alien", posterUrl }] }) } },
    );
    openApp("/libraries/1", transport);

    const link = await screen.findByRole("link", { name: "Alien" });
    expect(link.querySelector("img")?.getAttribute("srcset")).toBe(
      `${posterUrl}?width=320 320w, ${posterUrl}?width=480 480w`,
    );
  });

  test("loads from its own Server at only the widths its kind is made at", async () => {
    show([poster]);

    await expect(shownImage()).resolves.toStrictEqual({
      src: `${friend}/media/poster.jpg`,
      srcSet: `${friend}/media/poster.jpg?width=320 320w`,
    });
  });

  test("shows its thumbhash behind it until it loads", async () => {
    const { container } = show([poster]);
    await shownImage();

    const [preview, image] = container.querySelectorAll("img");
    expect([preview?.src.startsWith("data:image/png"), image?.src]).toStrictEqual([
      true,
      `${friend}/media/poster.jpg`,
    ]);
  });

  test("from a Server with no kind lists offers every width it resizes to", async () => {
    show([poster], { imageWidths: [320, 640] });

    const { srcSet } = await shownImage();
    expect(srcSet).toBe(
      `${friend}/media/poster.jpg?width=320 320w, ${friend}/media/poster.jpg?width=640 640w`,
    );
  });

  test("a Plugin logo offers no srcset", async () => {
    show([pluginLogoOf({ logoUrl: "/media/plugin-logos/tmdb/v1" })]);

    await expect(shownImage()).resolves.toStrictEqual({
      src: `${friend}/media/plugin-logos/tmdb/v1`,
      srcSet: null,
    });
  });
});

describe("an image's stand-ins", () => {
  test("a stand-in Backdrop offers backdropWidths", async () => {
    show([stillOf({}), backdrop]);

    const { srcSet } = await shownImage();
    expect(srcSet).toBe(`${friend}/media/backdrop.jpg?width=640 640w`);
  });

  test("a failed Trailer thumbnail falls back to the Backdrop", async () => {
    show([thumbnailOf({ thumbnailUrl: "/media/thumbnail.jpg" }), backdrop]);
    fireEvent.error(await screen.findByRole("img", { name: "Art" }));

    const { src } = await shownImage();
    expect(src).toBe(`${friend}/media/backdrop.jpg`);
  });

  test("once every one fails, the fallback shows", async () => {
    show([poster]);
    fireEvent.error(await screen.findByRole("img", { name: "Art" }));

    expect([document.body.textContent, document.querySelectorAll("img").length]).toStrictEqual([
      "Heat",
      0,
    ]);
  });

  test("a new image where one failed gets its own try", async () => {
    const { rerender } = show([poster]);
    fireEvent.error(await screen.findByRole("img", { name: "Art" }));

    const next = posterOf({ posterUrl: "/media/poster-2.jpg" });
    rerender(
      <ServerImg images={[next]} sizes="150px" className="size-full" alt="Art" fallback="Heat" />,
    );

    const { src } = await shownImage();
    expect(src).toBe(`${friend}/media/poster-2.jpg`);
  });

  test("without an image, the fallback shows without asking the Server its widths", () => {
    show([posterOf({})], null);

    expect(document.body.textContent).toBe("Heat");
  });
});
