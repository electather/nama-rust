import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";

import { CreateLibrary } from "@/components/library-form";
import { ScanButton, ScanStatus } from "@/components/library-scan";
import { Page } from "@/components/page";
import { type Library, LibraryService } from "@/gen/nama/v1/library_pb";
import { kindName } from "@/lib/libraries";

export const Route = createFileRoute("/_member/_admin/admin/libraries/")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
      staleTime: "static",
    }),
  component: Libraries,
});

function Libraries() {
  const { data } = useSuspenseQuery(LibraryService.method.listLibraries, {});
  return (
    <Page heading="Libraries">
      {data.libraries.length === 0 ? (
        <p className="mt-4 text-muted-foreground">No Libraries yet. Create one below.</p>
      ) : (
        // Pulled out by the rows' own padding, so the names line up with the heading.
        <ul className="mt-8 -mx-3 flex max-w-2xl flex-col">
          {data.libraries.map((library) => (
            <LibraryRow key={library.id} library={library} />
          ))}
        </ul>
      )}
      <section aria-labelledby="create-library" className="mt-12 max-w-sm">
        <h2 id="create-library" className="font-medium">
          Create a Library
        </h2>
        <CreateLibrary />
      </section>
    </Page>
  );
}

function LibraryRow({ library }: { library: Library }) {
  const folders = library.folders.length;
  return (
    <li className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-3 py-3 last:border-b-0">
      <div className="flex min-w-0 flex-col">
        <Link
          to="/admin/libraries/$libraryId"
          params={{ libraryId: String(library.id) }}
          className="truncate rounded-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {library.name}
        </Link>
        <span className="text-sm text-muted-foreground">
          {kindName(library.kind)} · {folders} {folders === 1 ? "folder" : "folders"}
        </span>
      </div>
      <div className="ms-auto flex items-center gap-3">
        <ScanStatus libraryId={library.id} />
        <ScanButton library={library} />
      </div>
    </li>
  );
}
