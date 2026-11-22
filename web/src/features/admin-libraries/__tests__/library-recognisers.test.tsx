import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, type LibrarySchema, type LibraryService } from "@/gen/nama/v1/library_pb";
import { ExtensionPoint, PluginService, type PluginSummarySchema } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { admin, fakeServer, fill } from "@/test-app";
import { refused } from "@/test-refusal";
import { openLibraries } from "@/test-settings";

afterEach(cleanup);

type Summary = MessageInitShape<typeof PluginSummarySchema>;

const recogniser = [ExtensionPoint.RECOGNISER];
const anime: Summary = {
  id: "anime",
  name: "Anime numbers",
  enabled: true,
  extensionPoints: recogniser,
};
const dates: Summary = { id: "dates", name: "", enabled: false, extensionPoints: recogniser };
const tvdb: Summary = {
  id: "tvdb",
  name: "TVDB",
  enabled: true,
  extensionPoints: [ExtensionPoint.TITLE_SOURCE],
};

const films: MessageInitShape<typeof LibrarySchema> = {
  id: 1n,
  name: "Films",
  kind: LibraryKind.MOVIES,
  folders: ["/srv/films"],
  titleSources: ["tmdb"],
};

/** A fake Server with `library`, listing `plugins`, answering Library calls with `calls`. */
function recognisersServer(
  calls: Partial<ServiceImpl<typeof LibraryService>>,
  plugins: Summary[] = [anime, dates, tvdb],
  library = films,
) {
  return fakeServer(
    {
      signedIn: admin,
      libraries: [library],
      libraryService: {
        getLibrary: () => ({ library }),
        listTitleSources: () => ({}),
        ...calls,
      },
    },
    {},
    {},
    (router) => {
      router.service(PluginService, { listPlugins: () => ({ summaries: plugins }) });
    },
  );
}

/** The Recognisers the field's list shows, in order, or none without a list. */
function recognisersShown() {
  const list = screen.queryByRole("list", { name: "Recognisers" });
  if (list === null) {
    return [];
  }
  return within(list)
    .getAllByRole("listitem")
    .map((item) => item.textContent);
}

/** Fills in a new Library's name and folder, then creates it. */
function createFilms() {
  fill("Name", "Films");
  fill("Folders", "/srv/films");
  fireEvent.click(screen.getByRole("button", { name: "Create Library" }));
}

test("a new Library sends the Recognisers picked, in their order, then opens", async () => {
  let sent: string[] = [];
  openLibraries(
    recognisersServer({
      createLibrary: (request) => {
        sent = request.recogniserIds;
        return { library: { ...films, recogniserIds: request.recogniserIds } };
      },
    }),
    "new",
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add Anime numbers" }));
  fireEvent.click(screen.getByRole("button", { name: "Add dates · not enabled" }));
  fireEvent.click(screen.getByRole("button", { name: "Move dates up" }));
  expect(screen.queryByRole("button", { name: /Add TVDB/u })).toBeNull();
  createFilms();

  await expect(screen.findByRole("heading", { name: "Films" })).resolves.toBeDefined();
  expect(sent).toStrictEqual(["dates", "anime"]);
});

test("a Library's Recognisers show in its order, and moving one saves the new order", async () => {
  let sent: string[] | undefined;
  openLibraries(
    recognisersServer(
      {
        updateLibrary: (request) => {
          sent = request.recogniserOrder?.ids;
          return { library: films };
        },
      },
      [anime, dates],
      { ...films, recogniserIds: ["dates", "gone", "anime"] },
    ),
    "1",
  );
  await screen.findByRole("button", { name: "Remove Anime numbers" });

  expect(recognisersShown()).toStrictEqual([
    "dates · not enabled",
    "gone · not available",
    "Anime numbers",
  ]);
  fireEvent.click(screen.getByRole("button", { name: "Move Anime numbers up" }));
  await waitFor(() => {
    expect(sent).toStrictEqual(["dates", "anime", "gone"]);
  });
});

test("removing a Library's last Recogniser saves an empty order, not none", async () => {
  let sent: string[] | undefined;
  openLibraries(
    recognisersServer(
      {
        updateLibrary: (request) => {
          sent = request.recogniserOrder?.ids;
          return { library: films };
        },
      },
      [anime],
      { ...films, recogniserIds: ["anime"] },
    ),
    "1",
  );
  fireEvent.click(await screen.findByRole("button", { name: "Remove Anime numbers" }));

  await waitFor(() => {
    expect(sent).toStrictEqual([]);
  });
});

test("with no Recogniser installed the field says the Server's own Matching reads every file", async () => {
  openLibraries(recognisersServer({}, [tvdb]), "1");

  const hint = await screen.findByText(
    "No Recogniser is installed, so the Server’s own Matching reads every file.",
  );
  const field = screen.getByRole("group", { name: "Recognisers" });
  expect(field.getAttribute("aria-describedby")).toContain(hint.id);
  expect(within(field).queryAllByRole("button")).toStrictEqual([]);
});

test("a refused create shows on the Recognisers field", async () => {
  openLibraries(
    recognisersServer({
      createLibrary: () => {
        throw refused(Code.InvalidArgument, "anime is named twice", {
          reason: RefusalReason.RECOGNISERS_INVALID,
          field: "recogniser_ids",
          params: { pluginId: "anime" },
        });
      },
    }),
    "new",
  );
  await screen.findByRole("button", { name: "Add Anime numbers" });
  createFilms();

  const error = await screen.findByText("Anime is named twice.");
  const field = screen.getByRole("group", { name: "Recognisers" });
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});

test("a refused Recogniser order says why in the toast, and the order goes back", async () => {
  openLibraries(
    recognisersServer({
      updateLibrary: () => {
        throw refused(Code.InvalidArgument, "dates is no Recogniser", {
          reason: RefusalReason.RECOGNISERS_INVALID,
          field: "recogniser_order",
          params: { pluginId: "dates" },
        });
      },
    }),
    "1",
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add Anime numbers" }));

  await expect(screen.findByText("Dates is no Recogniser.")).resolves.toBeDefined();
  expect(screen.getByText("Not saved")).toBeDefined();
  expect(recognisersShown()).toStrictEqual([]);
});

const introDb: Summary = {
  id: "theintrodb",
  name: "TheIntroDB",
  enabled: true,
  extensionPoints: [ExtensionPoint.SEGMENT_SOURCE],
};
const chapters: Summary = {
  id: "chapters",
  name: "",
  enabled: true,
  extensionPoints: [ExtensionPoint.SEGMENT_SOURCE],
};

test("a new Library sends its Segment sources in their order, apart from its Recognisers", async () => {
  let sent: { recognisers: string[]; sources: string[] } | undefined;
  openLibraries(
    recognisersServer(
      {
        createLibrary: (request) => {
          sent = { recognisers: request.recogniserIds, sources: request.segmentSourceIds };
          return { library: films };
        },
      },
      [anime, introDb, chapters],
    ),
    "new",
  );
  const sources = await screen.findByRole("group", { name: "Segment sources" });
  fireEvent.click(await within(sources).findByRole("button", { name: "Add TheIntroDB" }));
  fireEvent.click(within(sources).getByRole("button", { name: "Add chapters" }));
  fireEvent.click(within(sources).getByRole("button", { name: "Move chapters up" }));
  expect(within(sources).queryByRole("button", { name: /Anime/u })).toBeNull();
  createFilms();

  await expect(screen.findByRole("heading", { name: "Films" })).resolves.toBeDefined();
  expect(sent).toStrictEqual({ recognisers: [], sources: ["chapters", "theintrodb"] });
});

test("removing a Library's Segment source saves the order without it", async () => {
  let sent: string[] | undefined;
  openLibraries(
    recognisersServer(
      {
        updateLibrary: (request) => {
          sent = request.segmentSourceOrder?.ids;
          return { library: films };
        },
      },
      [introDb, chapters],
      { ...films, segmentSourceIds: ["theintrodb", "chapters"] },
    ),
    "1",
  );
  fireEvent.click(await screen.findByRole("button", { name: "Remove TheIntroDB" }));

  await waitFor(() => {
    expect(sent).toStrictEqual(["chapters"]);
  });
});

test("a refused Segment source order says why in the toast", async () => {
  openLibraries(
    recognisersServer(
      {
        updateLibrary: () => {
          throw refused(Code.InvalidArgument, "theintrodb is no Segment source", {
            reason: RefusalReason.SEGMENT_SOURCES_INVALID,
            field: "segment_source_order",
            params: { pluginId: "theintrodb" },
          });
        },
      },
      [introDb],
    ),
    "1",
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add TheIntroDB" }));

  await expect(screen.findByText("Theintrodb is no Segment source.")).resolves.toBeDefined();
  expect(screen.getByText("Not saved")).toBeDefined();
});
