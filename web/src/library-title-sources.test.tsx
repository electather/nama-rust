import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { admin, fakeServer, fill, openApp } from "./test-app";
import { refused } from "./test-refusal";

afterEach(cleanup);

const tmdb = { namespace: "tmdb", pluginId: "tmdb", pluginName: "TMDB", searches: true };
const tvdb = { namespace: "tvdb", pluginId: "tvdb", pluginName: "TVDB", searches: true };
const fanart = { namespace: "fanart", pluginId: "fanart", pluginName: "fanart.tv" };

/**
 * A fake Server with the Library Films, ordered `titleSources`, whose saved orders go to `saved`,
 * and whose running title sources are `running`.
 */
function titleSourcesServer(
  titleSources: string[],
  saved: (titleSources: string[]) => void = () => {},
  running = [tmdb, tvdb, fanart],
) {
  const library = {
    id: 1n,
    name: "Films",
    kind: LibraryKind.MOVIES,
    folders: ["/srv/films"],
    titleSources,
  };
  return fakeServer({
    signedIn: admin,
    events: new ReadableStream(),
    libraries: [library],
    libraryService: {
      getLibrary: () => ({ library }),
      listTitleSources: () => ({ titleSources: running }),
      updateLibrary: (request) => {
        saved(request.titleSources);
        return { library: { ...library, titleSources: request.titleSources } };
      },
      createLibrary: () => {
        throw refused(Code.InvalidArgument, "tmdb is named twice", {
          reason: RefusalReason.TITLE_SOURCES_INVALID,
          field: "title_sources",
        });
      },
    },
  });
}
/** The names the title sources list shows, in order. */
function titleSourcesShown() {
  const list = screen.getByRole("list", { name: "Title sources" });
  return within(list)
    .getAllByRole("listitem")
    .map((item) => item.textContent);
}

test("a Library's title sources show in its order, and save unchanged", async () => {
  let saved: string[] = [];
  openApp(
    "/admin/libraries/1",
    titleSourcesServer(["tvdb", "imdb", "tmdb"], (sources) => {
      saved = sources;
    }),
  );
  await screen.findByRole("button", { name: "Remove TVDB" });

  expect(titleSourcesShown()).toEqual(["TVDB", "imdb · not available", "TMDB"]);
  fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  await screen.findByText("Saved. Scan the Library to pick up new folders.");
  expect(saved).toEqual(["tvdb", "imdb", "tmdb"]);
});

test("moving a title source up saves the new order", async () => {
  let saved: string[] = [];
  openApp(
    "/admin/libraries/1",
    titleSourcesServer(["tmdb"], (sources) => {
      saved = sources;
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add TVDB" }));
  fireEvent.click(screen.getByRole("button", { name: "Move TVDB up" }));
  fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

  await screen.findByText("Saved. Scan the Library to pick up new folders.");
  expect(saved).toEqual(["tvdb", "tmdb"]);
});

test("the last title source can't be removed", async () => {
  openApp("/admin/libraries/1", titleSourcesServer(["tvdb", "tmdb"]));
  fireEvent.click(await screen.findByRole("button", { name: "Remove TVDB" }));

  expect(titleSourcesShown()).toEqual(["TMDB"]);
  expect(screen.getByRole("button", { name: "Remove TMDB" }).hasAttribute("disabled")).toBe(true);
});

test("a refused title source order shows under the field", async () => {
  openApp("/admin/libraries", titleSourcesServer(["tmdb"]));
  await screen.findByRole("button", { name: "Create Library" });
  fill("Name", "Films");
  fireEvent.change(screen.getByRole("textbox", { name: "Folders" }), {
    target: { value: "/srv/films" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Create Library" }));

  const error = await screen.findByText("Tmdb is named twice.");
  const field = screen.getByRole("group", { name: "Title sources" });
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});

test("creating a Library puts the title sources back to TMDB alone", async () => {
  openApp(
    "/admin/libraries",
    fakeServer({
      signedIn: admin,
      events: new ReadableStream(),
      libraries: [],
      libraryService: {
        listTitleSources: () => ({ titleSources: [tmdb, tvdb] }),
        createLibrary: (request) => ({
          library: { id: 1n, name: request.name, kind: request.kind, folders: request.folders },
        }),
      },
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add TVDB" }));
  fireEvent.click(screen.getByRole("button", { name: "Move TVDB up" }));
  fill("Name", "Shows");
  fireEvent.change(screen.getByRole("textbox", { name: "Folders" }), {
    target: { value: "/srv/shows" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Create Library" }));

  await screen.findByText("Shows is ready. Scan it to fill it.");
  expect(titleSourcesShown()).toEqual(["TMDB"]);
});

test("a source that doesn't search shows as artwork only, in its row and on its add button", async () => {
  openApp("/admin/libraries/1", titleSourcesServer(["tmdb"]));
  const add = await screen.findByRole("button", { name: "Add fanart.tv · Artwork only" });
  fireEvent.click(add);

  expect(titleSourcesShown()).toEqual(["TMDB", "fanart.tv · Artwork only"]);
  screen.getByText(/artwork comes from every source in the order/iu);
});

test("an order where no running source searches says so, and links to the Plugins", async () => {
  openApp(
    "/admin/libraries/1",
    titleSourcesServer(["tvdb", "fanart"], () => {}, [tmdb, fanart]),
  );
  await screen.findByRole("button", { name: "Remove fanart.tv" });

  const note = await screen.findByText(/no source above can search for titles/iu);
  expect(titleSourcesShown()).toEqual(["tvdb · not available", "fanart.tv · Artwork only"]);
  const plugins = within(note).getByRole("link", { name: "Plugins" });
  expect(plugins.getAttribute("href")).toBe("/admin/plugins");
});

test("an order with a running source that searches says nothing of it", async () => {
  openApp("/admin/libraries/1", titleSourcesServer(["fanart", "tmdb"]));
  await screen.findByRole("button", { name: "Remove TMDB" });

  expect(screen.queryByText(/no source above can search for titles/iu)).toBeNull();
});
