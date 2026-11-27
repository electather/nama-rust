import type { MessageInitShape } from "@bufbuild/protobuf";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, assert, expect, test } from "vite-plus/test";

import { BrowseService, type MovieSchema } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { fakeServer, newUser, openApp } from "@/test-app";

afterEach(cleanup);

/** Opens the page of `movie`, Parasite with one Version unless it says otherwise, in Library 1. */
function openMovie(movie: MessageInitShape<typeof MovieSchema>) {
  const parasite = { title: "Parasite", versions: [{ id: 1n, fileName: "Parasite.mkv" }] };
  const libraries = [{ id: 1n, name: "Films", kind: LibraryKind.MOVIES }];
  const server = fakeServer({ signedIn: newUser, libraries }, {}, {}, (router) => {
    router.service(BrowseService, { getMovie: () => ({ movie: { ...parasite, ...movie } }) });
  });
  openApp("/libraries/1/movies/tmdb-496243", server);
}

test("a Movie's Trailers load nothing from their host until one is played in a pop-up", async () => {
  const youtube = "https://www.youtube.com/watch?v=isOGD_7hNIY";
  const vimeo = "https://vimeo.com/123456789";
  openMovie({
    trailers: [
      { name: "Official Trailer", url: youtube },
      { name: "Teaser", url: vimeo },
    ],
  });

  const row = await screen.findByRole("region", { name: "Trailers" });
  const links = within(row).getAllByRole("link");
  expect(links.map((link) => link.getAttribute("href"))).toStrictEqual([youtube, vimeo]);
  expect(document.querySelector("iframe, video, [src*='youtube'], [src*='vimeo']")).toBeNull();

  fireEvent.click(within(row).getByRole("link", { name: "Official Trailer" }));

  const player = within(screen.getByRole("dialog", { name: "Official Trailer" })).getByTitle(
    "Official Trailer",
  );
  expect(player.getAttribute("src")).toBe(
    "https://www.youtube-nocookie.com/embed/isOGD_7hNIY?autoplay=1",
  );
});

test("right-clicking a Trailer opens the same menu as its …", async () => {
  openMovie({ trailers: [{ name: "Teaser", url: "https://vimeo.com/123456789" }] });

  const row = await screen.findByRole("region", { name: "Trailers" });
  fireEvent.contextMenu(within(row).getByRole("link", { name: "Teaser" }));

  await expect(screen.findByRole("menuitem", { name: "Share" })).resolves.toBeDefined();
});

test("a title without Trailers has no Trailers row", async () => {
  openMovie({});

  await expect(screen.findByRole("heading", { level: 1, name: "Parasite" })).resolves.toBeDefined();
  expect(screen.queryByRole("region", { name: "Trailers" })).toBeNull();
});

test("a Trailer's card shows its own thumbnail from the Server, and the title's Backdrop without one", async () => {
  const thumbnail =
    "/media/artwork/tmdb/https%3A%2F%2Fi.ytimg.com%2Fvi%2FisOGD_7hNIY%2Fmaxresdefault.jpg";
  const backdrop = "/media/artwork/tmdb/%2Fparasite-backdrop.jpg";
  openMovie({
    backdropUrl: backdrop,
    trailers: [
      {
        name: "Official Trailer",
        url: "https://www.youtube.com/watch?v=isOGD_7hNIY",
        thumbnailUrl: thumbnail,
      },
      { name: "Teaser", url: "https://vimeo.com/123456789" },
    ],
  });

  const row = await screen.findByRole("region", { name: "Trailers" });
  const art = [...row.querySelectorAll("li img")].map((image) => image.getAttribute("src"));
  expect(art).toStrictEqual([thumbnail, backdrop]);
});

test("a Trailer's card whose thumbnail fails to load shows the title's Backdrop", async () => {
  const backdrop = "/media/artwork/tmdb/%2Fparasite-backdrop.jpg";
  openMovie({
    backdropUrl: backdrop,
    trailers: [
      {
        name: "Official Trailer",
        url: "https://www.youtube.com/watch?v=isOGD_7hNIY",
        thumbnailUrl:
          "/media/artwork/tmdb/https%3A%2F%2Fi.ytimg.com%2Fvi%2FisOGD_7hNIY%2Fmaxresdefault.jpg",
      },
    ],
  });
  const row = await screen.findByRole("region", { name: "Trailers" });
  const thumbnail = row.querySelector("li img");
  assert(thumbnail);

  fireEvent.error(thumbnail);

  expect(row.querySelector("li img")?.getAttribute("src")).toBe(backdrop);
});
