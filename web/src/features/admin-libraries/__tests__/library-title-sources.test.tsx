import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, type LibrarySchema } from "@/gen/nama/v1/library_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { admin, fakeServer, fill } from "@/test-app";
import { refused } from "@/test-refusal";
import { openLibraries } from "@/test-settings";

afterEach(cleanup);

const tmdb = {
  namespace: "tmdb",
  pluginId: "tmdb",
  pluginName: "TMDB",
  searches: true,
  running: true,
};
const tvdb = {
  namespace: "tvdb",
  pluginId: "tvdb",
  pluginName: "TVDB",
  searches: true,
  running: true,
};
const fanart = { namespace: "fanart", pluginId: "fanart", pluginName: "fanart.tv", running: true };
const tvdbDisabled = { ...tvdb, running: false };

/**
 * A fake Server with the Library Films, ordered `titleSources`, whose saved orders go to `saved`,
 * whose installed title sources are `listed`, and where `signedIn` is signed in.
 */
function titleSourcesServer(
  titleSources: string[],
  {
    saved = () => {},
    listed = [tmdb, tvdb, fanart],
    signedIn = admin,
  }: {
    saved?: (titleSources: string[]) => void;
    listed?: {
      namespace: string;
      pluginId: string;
      pluginName: string;
      searches?: boolean;
      running: boolean;
    }[];
    signedIn?: Permission[];
  } = {},
) {
  let library = {
    id: 1n,
    name: "Films",
    kind: LibraryKind.MOVIES,
    folders: ["/srv/films"],
    titleSources,
  };
  return fakeServer({
    signedIn,
    events: new ReadableStream(),
    libraryService: {
      listLibraries: () => ({ libraries: [library] }),
      getLibrary: () => ({ library }),
      listTitleSources: () => ({ titleSources: listed }),
      updateLibrary: (request) => {
        saved(request.titleSources);
        library = { ...library, titleSources: request.titleSources };
        return { library };
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
    .map((item) => Array.from(item.querySelectorAll("p"), (line) => line.textContent).join(" · "));
}

test("a Library's title sources show in its order, and removing one saves the rest as they are", async () => {
  let saved: string[] = [];
  openLibraries(
    titleSourcesServer(["tvdb", "imdb", "tmdb"], {
      saved: (sources) => {
        saved = sources;
      },
    }),
    "1",
  );
  await screen.findByRole("button", { name: "Remove TVDB" });

  expect(titleSourcesShown()).toStrictEqual(["TVDB", "imdb · Not available", "TMDB"]);
  fireEvent.click(screen.getByRole("button", { name: "Remove TVDB" }));
  await waitFor(() => {
    expect(saved).toStrictEqual(["imdb", "tmdb"]);
  });
});

test("moving a title source up saves the new order", async () => {
  let saved: string[] = [];
  openLibraries(
    titleSourcesServer(["tmdb"], {
      saved: (sources) => {
        saved = sources;
      },
    }),
    "1",
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add TVDB" }));
  fireEvent.click(await screen.findByRole("button", { name: "Move TVDB up" }));

  await waitFor(() => {
    expect(saved).toStrictEqual(["tvdb", "tmdb"]);
  });
});

test("the last title source can't be removed", async () => {
  openLibraries(titleSourcesServer(["tvdb", "tmdb"]), "1");
  fireEvent.click(await screen.findByRole("button", { name: "Remove TVDB" }));

  expect(titleSourcesShown()).toStrictEqual(["TMDB"]);
  expect(screen.getByRole("button", { name: "Remove TMDB" }).hasAttribute("disabled")).toBe(true);
});

test("a refused title source order shows under the field", async () => {
  openLibraries(titleSourcesServer(["tmdb"]), "new");
  await screen.findByRole("button", { name: "Create Library" });
  fill("Name", "Films");
  fill("Folders", "/srv/films");
  fireEvent.click(screen.getByRole("button", { name: "Create Library" }));

  const error = await screen.findByText("Tmdb is named twice.");
  const field = screen.getByRole("group", { name: "Title sources" });
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});

test("a new Library starts with TMDB alone, and creating it opens it", async () => {
  const libraries: MessageInitShape<typeof LibrarySchema>[] = [];
  openLibraries(
    fakeServer({
      signedIn: admin,
      events: new ReadableStream(),
      libraries,
      libraryService: {
        listTitleSources: () => ({ titleSources: [tmdb, tvdb] }),
        createLibrary: ({ name, kind, folders }) => {
          libraries.push({ id: 1n, name, kind, folders });
          return { library: libraries[0] };
        },
        getLibrary: () => ({ library: libraries[0] }),
      },
    }),
    "new",
  );
  await screen.findByRole("button", { name: "Add TVDB" });
  expect(titleSourcesShown()).toStrictEqual(["TMDB"]);
  fill("Name", "Shows");
  fill("Folders", "/srv/shows");
  fireEvent.click(screen.getByRole("button", { name: "Create Library" }));

  await expect(screen.findByRole("heading", { name: "Shows" })).resolves.toBeDefined();
  const crumbs = screen.getByRole("navigation", { name: "Breadcrumbs" });
  expect(crumbs.textContent).toBe("LibrariesShows");
});

test("a source that doesn't search shows as artwork only, in its row and on its add button", async () => {
  openLibraries(titleSourcesServer(["tmdb"]), "1");
  const add = await screen.findByRole("button", { name: "Add fanart.tv · Artwork only" });
  fireEvent.click(add);

  expect(titleSourcesShown()).toStrictEqual(["TMDB", "fanart.tv · Artwork only"]);
  screen.getByText(/artwork comes from every source in the order/iu);
});

test("an order where no running source searches names why each can't, and links to the Plugins", async () => {
  openLibraries(titleSourcesServer(["tvdb", "fanart"], { listed: [tmdb, fanart] }), "1");
  await screen.findByRole("button", { name: "Remove fanart.tv" });

  const note = await screen.findByText(/no source above can search for titles/iu);
  expect(note.textContent).toContain(
    "No source above can search for titles: tvdb isn’t running and fanart.tv only adds artwork.",
  );
  const plugins = within(note).getByRole("link", { name: "Plugins" });
  expect(plugins.getAttribute("href")).toBe("/code?settings=plugins");
});

test("the note names the Plugins without a link for a User who can't manage them", async () => {
  openLibraries(
    titleSourcesServer(["tvdb"], { listed: [tmdb], signedIn: [Permission.MANAGE_LIBRARIES] }),
    "1",
  );

  const note = await screen.findByText(/no source above can search for titles/iu);
  expect(note.textContent).toContain("under Plugins");
  expect(within(note).queryByRole("link")).toBeNull();
});

test("an order with a running source that searches says nothing of it", async () => {
  openLibraries(titleSourcesServer(["fanart", "tmdb"]), "1");
  await screen.findByRole("button", { name: "Remove TMDB" });

  expect(screen.queryByText(/no source above can search for titles/iu)).toBeNull();
});

test("a stored source whose Plugin isn't running shows by the Plugin's name", async () => {
  openLibraries(
    titleSourcesServer(["tmdb", "tvdb", "imdb"], { listed: [tmdb, tvdbDisabled] }),
    "1",
  );
  await screen.findByRole("button", { name: "Remove TVDB" });

  expect(titleSourcesShown()).toStrictEqual(["TMDB", "TVDB · Not running", "imdb · Not available"]);
  screen.getByRole("button", { name: "Move TVDB up" });
});

test("a source that isn't running can't be added", async () => {
  openLibraries(titleSourcesServer(["tmdb"], { listed: [tmdb, tvdbDisabled] }), "1");
  await screen.findByRole("button", { name: "Remove TMDB" });

  expect(screen.queryByRole("button", { name: /^Add/u })).toBeNull();
});

test("a searching source that isn't running doesn't count as one that searches", async () => {
  openLibraries(titleSourcesServer(["tvdb"], { listed: [tmdb, tvdbDisabled] }), "1");

  const note = await screen.findByText(/no source above can search for titles/iu);
  expect(note.textContent).toContain("No source above can search for titles: TVDB isn’t running.");
});
