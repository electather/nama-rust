import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQueries } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useId, useState } from "react";

import { Page } from "@/components/page";
import { UnmatchedRow } from "@/components/unmatched-item";
import { type Library, LibraryService } from "@/gen/nama/v1/library_pb";
import { type ListUnmatchedItemsResponse, MatchService } from "@/gen/nama/v1/match_pb";

const librariesQuery = (transport: Transport) =>
  createQueryOptions(LibraryService.method.listLibraries, {}, { transport });
const unmatchedQuery = (library: Library, transport: Transport) =>
  createQueryOptions(
    MatchService.method.listUnmatchedItems,
    { libraryId: library.id },
    { transport },
  );

export const Route = createFileRoute("/_member/_admin/admin/needs-a-match")({
  validateSearch: (search: Record<string, unknown>): { library?: string } =>
    typeof search["library"] === "number" || typeof search["library"] === "string"
      ? { library: String(search["library"]) }
      : {},
  loader: async ({ context: { queryClient, transport } }) => {
    const { libraries } = await queryClient.query({
      ...librariesQuery(transport),
      staleTime: "static",
    });
    await Promise.all(
      libraries.map((library) =>
        queryClient.query({ ...unmatchedQuery(library, transport), staleTime: "static" }),
      ),
    );
  },
  component: NeedsAMatch,
});

function NeedsAMatch() {
  const transport = useTransport();
  const { libraries } = useSuspenseQuery(LibraryService.method.listLibraries, {}).data;
  const { library: picked } = Route.useSearch();
  const shown = libraries.filter(
    (library) => picked === undefined || String(library.id) === picked,
  );
  const lists = useSuspenseQueries({
    queries: shown.map((library) => unmatchedQuery(library, transport)),
  });
  const needing = shown
    .map((library, index) => ({ library, list: lists[index]?.data }))
    .filter(({ list }) => list !== undefined && list.items.length + list.unparsedPaths.length > 0);
  // One Item at a time is being fixed, so one Search button is on screen.
  const [fixing, setFixing] = useState<bigint>();

  return (
    <Page heading="Needs a Match">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Files the Server couldn’t Match to a Movie or Show. Find the right title for each, and it
        stays Matched.
      </p>
      {libraries.length > 1 && <LibraryFilter libraries={libraries} picked={picked} />}
      {needing.length === 0 ? (
        <p className="mt-10 font-medium">Everything is Matched.</p>
      ) : (
        needing.map(({ library, list }) => (
          <LibrarySection key={library.id} library={library} list={list}>
            {list?.items.map((item) => (
              <UnmatchedRow
                key={item.itemId}
                item={item}
                libraryId={library.id}
                open={fixing === item.itemId}
                onToggle={() => {
                  setFixing(fixing === item.itemId ? undefined : item.itemId);
                }}
              />
            ))}
          </LibrarySection>
        ))
      )}
    </Page>
  );
}

function LibraryFilter({
  libraries,
  picked,
}: {
  libraries: Library[];
  picked: string | undefined;
}) {
  const navigate = Route.useNavigate();
  const id = useId();
  return (
    <div className="mt-8 flex items-center gap-3">
      <label htmlFor={id} className="text-sm font-medium">
        Library
      </label>
      <select
        id={id}
        value={picked ?? ""}
        onChange={(event) => {
          const library = event.target.value;
          void navigate({ search: library === "" ? {} : { library } });
        }}
        className="h-9 rounded-lg border border-input bg-background px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25"
      >
        <option value="">All Libraries</option>
        {libraries.map((library) => (
          <option key={library.id} value={String(library.id)}>
            {library.name}
          </option>
        ))}
      </select>
    </div>
  );
}

function LibrarySection({
  library,
  list,
  children,
}: {
  library: Library;
  list: ListUnmatchedItemsResponse | undefined;
  children: React.ReactNode;
}) {
  const heading = useId();
  return (
    <section aria-labelledby={heading} className="mt-10 max-w-3xl">
      <h2 id={heading} className="font-medium">
        {library.name}
      </h2>
      {/* Pulled out by the rows' own padding, so the titles line up with the heading. */}
      <ul className="-mx-3 mt-2 flex flex-col">{children}</ul>
      {list && list.unparsedPaths.length > 0 && (
        <div className="mt-6">
          <h3 className="text-sm font-medium">Files to rename</h3>
          <p className="mt-1 text-sm text-pretty text-muted-foreground">
            Their names don’t say which Movie or Episode they are. Rename them, then scan the
            Library.
          </p>
          <ul className="mt-2 flex flex-col gap-1 text-sm wrap-anywhere">
            {list.unparsedPaths.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
