import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { type ReactNode, useId, useState } from "react";

import { Page } from "@/components/page";
import { ShowMore } from "@/components/show-more";
import { UnmatchedRow } from "@/components/unmatched-item";
import { type Library, LibraryService } from "@/gen/nama/v1/library_pb";
import {
  type ListUnmatchedItemsResponse,
  MatchService,
  type UnmatchedItem,
} from "@/gen/nama/v1/match_pb";
import { pagedQuery, usePaged } from "@/lib/paged";

const librariesQuery = (transport: Transport) =>
  createQueryOptions(LibraryService.method.listLibraries, {}, { transport });

/** `ListUnmatchedItems`' first page of `library`'s Unmatched Items. */
function firstPageOf(library: Library) {
  return { filter: { libraryId: library.id }, page: { token: "" } };
}

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
        queryClient.infiniteQuery({
          ...pagedQuery(MatchService.method.listUnmatchedItems, firstPageOf(library), transport),
          staleTime: "static",
        }),
      ),
    );
  },
  component: NeedsAMatch,
});

function NeedsAMatch() {
  const { libraries } = useSuspenseQuery(LibraryService.method.listLibraries, {}).data;
  const { library: picked } = Route.useSearch();
  const shown = pickedLibraries(libraries, picked);

  return (
    <Page heading="Needs a Match">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Files the Server couldn’t Match to a Movie or Show. Find the right title for each, and it
        stays Matched.
      </p>
      {(libraries.length > 1 || !shown) && <LibraryFilter libraries={libraries} picked={picked} />}
      {shown ? (
        <UnmatchedLists libraries={shown} />
      ) : (
        <p className="mt-10 font-medium">That Library isn’t on the Server.</p>
      )}
    </Page>
  );
}

/** The Libraries the filter `picked` shows: every one without it, none if it names no Library. */
function pickedLibraries(libraries: Library[], picked: string | undefined) {
  if (picked === undefined) return libraries;
  const library = libraries.find(({ id }) => String(id) === picked);
  return library && [library];
}

/** The Unmatched Items of each of `libraries` that has any, or that everything is Matched. */
function UnmatchedLists({ libraries }: { libraries: Library[] }) {
  // One Item at a time is being fixed, so one Search button is on screen.
  const [fixing, setFixing] = useState<bigint>();
  const toggle = (itemId: bigint) => {
    setFixing(fixing === itemId ? undefined : itemId);
  };
  return <ListsFrom libraries={libraries} anyBefore={false} fixing={fixing} toggle={toggle} />;
}

/** Which Item is being fixed, and what opens or closes one. */
interface Fixing {
  fixing: bigint | undefined;
  toggle: (itemId: bigint) => void;
}

/**
 * The Unmatched Items of the first of `libraries` if it has any, then the rest's, or that
 * everything is Matched if neither they nor the Libraries before them (`anyBefore`) have any.
 * Each Library is a component of its own, since each pages through its list with a hook.
 */
function ListsFrom({
  libraries: [library, ...rest],
  anyBefore,
  ...fixing
}: { libraries: Library[]; anyBefore: boolean } & Fixing) {
  if (library === undefined) {
    return anyBefore ? undefined : <p className="mt-10 font-medium">Everything is Matched.</p>;
  }
  return <LibraryList library={library} rest={rest} anyBefore={anyBefore} {...fixing} />;
}

/** `library`'s Unmatched Items a page at a time, if it has any, then the `rest`'s. */
function LibraryList({
  library,
  rest,
  anyBefore,
  fixing,
  toggle,
}: { library: Library; rest: Library[]; anyBefore: boolean } & Fixing) {
  const { data, hasNextPage, fetchNextPage } = usePaged(
    MatchService.method.listUnmatchedItems,
    firstPageOf(library),
  );
  const { items, unparsedPaths, needsAMatch } = loaded(data.pages, hasNextPage);
  return (
    <>
      {needsAMatch && (
        <LibrarySection library={library} unparsedPaths={unparsedPaths}>
          <UnmatchedRows items={items} libraryId={library.id} fixing={fixing} toggle={toggle} />
          {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
        </LibrarySection>
      )}
      <ListsFrom
        libraries={rest}
        anyBefore={anyBefore || needsAMatch}
        fixing={fixing}
        toggle={toggle}
      />
    </>
  );
}

/**
 * The Items and unparsed paths `pages` list, and whether their Library needs a Match: pages yet to
 * load (`hasNextPage`) count too, as an Event can take every loaded Item off before they load.
 */
function loaded(pages: ListUnmatchedItemsResponse[], hasNextPage: boolean) {
  const items = pages.flatMap((page) => page.items);
  // ponytail: only the first page carries them, and `usePaged` lets it go after 10 pages; keep
  // them as it keeps `totalSize` if a Library that long turns up.
  const unparsedPaths = pages.flatMap((page) => page.unparsedPaths);
  return {
    items,
    unparsedPaths,
    needsAMatch: items.length + unparsedPaths.length > 0 || hasNextPage,
  };
}

/** `items`, the Unmatched Items of the Library `libraryId`, each with its Find a Match. */
function UnmatchedRows({
  items,
  libraryId,
  fixing,
  toggle,
}: { items: UnmatchedItem[]; libraryId: bigint } & Fixing) {
  return (
    // Pulled out by the rows' own padding, so the titles line up with the heading.
    <ul className="-mx-3 mt-2 flex flex-col">
      {items.map((item) => (
        <UnmatchedRow
          key={item.itemId}
          item={item}
          libraryId={libraryId}
          open={fixing === item.itemId}
          onToggle={() => {
            toggle(item.itemId);
          }}
        />
      ))}
    </ul>
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
        className="h-9 rounded-lg border border-input bg-transparent px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/25"
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
  unparsedPaths,
  children,
}: {
  library: Library;
  unparsedPaths: string[];
  children: ReactNode;
}) {
  const heading = useId();
  return (
    <section aria-labelledby={heading} className="mt-10 max-w-3xl">
      <h2 id={heading} className="font-medium">
        {library.name}
      </h2>
      {children}
      {unparsedPaths.length > 0 && (
        <div className="mt-6">
          <h3 className="text-sm font-medium">Files to rename</h3>
          <p className="mt-1 text-sm text-pretty text-muted-foreground">
            Their names don’t say which Movie or Episode they are. Rename them, then scan the
            Library.
          </p>
          <ul className="mt-2 flex flex-col gap-1 text-sm wrap-anywhere">
            {unparsedPaths.map((path) => (
              <li key={path}>{path}</li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
