import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, type ServiceImpl } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { LibraryKind, type LibrarySchema, type LibraryService } from "@/gen/nama/v1/library_pb";
import { ExtensionPoint, PluginService, type PluginSummarySchema } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";

import { admin, fakeServer, fill, openApp } from "./test-app";
import { refused } from "./test-refusal";

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
  if (list === null) return [];
  return within(list)
    .getAllByRole("listitem")
    .map((item) => item.textContent);
}

/** Fills in a new Library's name and folder, then creates it. */
function createFilms() {
  fill("Name", "Films");
  fireEvent.change(screen.getByRole("textbox", { name: "Folders" }), {
    target: { value: "/srv/films" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Create Library" }));
}

test("a new Library sends the Recognisers picked, in their order, then the field empties", async () => {
  let sent: string[] = [];
  openApp(
    "/admin/libraries",
    recognisersServer({
      createLibrary: (request) => {
        sent = request.recogniserIds;
        return { library: { ...films, recogniserIds: request.recogniserIds } };
      },
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Add Anime numbers" }));
  fireEvent.click(screen.getByRole("button", { name: "Add dates · not enabled" }));
  fireEvent.click(screen.getByRole("button", { name: "Move dates up" }));
  expect(screen.queryByRole("button", { name: /Add TVDB/u })).toBeNull();
  createFilms();

  await screen.findByText("Films is ready. Scan it to fill it.");
  expect(sent).toEqual(["dates", "anime"]);
  expect(recognisersShown()).toEqual([]);
});

test("a Library's Recognisers show in its order, and save unchanged", async () => {
  let sent: string[] | undefined;
  openApp(
    "/admin/libraries/1",
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
  );
  await screen.findByRole("button", { name: "Remove Anime numbers" });

  expect(recognisersShown()).toEqual([
    "dates · not enabled",
    "gone · not available",
    "Anime numbers",
  ]);
  fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
  await screen.findByText("Saved. Scan the Library to pick up new folders.");
  expect(sent).toEqual(["dates", "gone", "anime"]);
});

test("removing a Library's last Recogniser saves an empty order, not none", async () => {
  let sent: string[] | undefined;
  openApp(
    "/admin/libraries/1",
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
  );
  fireEvent.click(await screen.findByRole("button", { name: "Remove Anime numbers" }));
  fireEvent.click(screen.getByRole("button", { name: "Save changes" }));

  await screen.findByText("Saved. Scan the Library to pick up new folders.");
  expect(sent).toEqual([]);
});

test("with no Recogniser installed the field says the Server's own Matching reads every file", async () => {
  openApp("/admin/libraries/1", recognisersServer({}, [tvdb]));

  const hint = await screen.findByText(
    "No Recogniser is installed, so the Server’s own Matching reads every file.",
  );
  const field = screen.getByRole("group", { name: "Recognisers" });
  expect(field.getAttribute("aria-describedby")).toContain(hint.id);
  expect(within(field).queryAllByRole("button")).toEqual([]);
});

test("a refused create shows on the Recognisers field", async () => {
  openApp(
    "/admin/libraries",
    recognisersServer({
      createLibrary: () => {
        throw refused(Code.InvalidArgument, "anime is named twice", {
          reason: RefusalReason.RECOGNISERS_INVALID,
          field: "recogniser_ids",
          params: { pluginId: "anime" },
        });
      },
    }),
  );
  await screen.findByRole("button", { name: "Add Anime numbers" });
  createFilms();

  const error = await screen.findByText("Anime is named twice.");
  const field = screen.getByRole("group", { name: "Recognisers" });
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});

test("a refused update shows on the Recognisers field", async () => {
  openApp(
    "/admin/libraries/1",
    recognisersServer({
      updateLibrary: () => {
        throw refused(Code.InvalidArgument, "dates is no Recogniser", {
          reason: RefusalReason.RECOGNISERS_INVALID,
          field: "recogniser_order",
          params: { pluginId: "dates" },
        });
      },
    }),
  );
  fireEvent.click(await screen.findByRole("button", { name: "Save changes" }));

  const error = await screen.findByText("Dates is no Recogniser.");
  const field = screen.getByRole("group", { name: "Recognisers" });
  expect(field.getAttribute("aria-describedby")).toContain(error.id);
});
