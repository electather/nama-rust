import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { DeleteLibrary, EditLibrary } from "@/components/library-form";
import { ScanButton, ScanStatus } from "@/components/library-scan";
import { Page } from "@/components/page";
import { NotFound } from "@/components/route-error";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { queryOrNotFound } from "@/lib/browse";
import { kindName } from "@/lib/libraries";

/** The Library in the URL's id, or "not found" for one that isn't a number. */
function libraryInput(libraryId: string) {
  if (!/^\d+$/u.test(libraryId)) throw notFound();
  return { id: BigInt(libraryId) };
}

export const Route = createFileRoute("/_member/_admin/admin/libraries/$libraryId")({
  loader: ({ context, params }) =>
    queryOrNotFound(context, LibraryService.method.getLibrary, libraryInput(params.libraryId)),
  notFoundComponent: NotFound,
  component: LibraryPage,
});

function LibraryPage() {
  const { libraryId } = Route.useParams();
  const { library } = useSuspenseQuery(
    LibraryService.method.getLibrary,
    libraryInput(libraryId),
  ).data;
  if (!library) throw notFound();
  return (
    <Page heading={library.name}>
      <p className="mt-1 text-sm text-muted-foreground">{kindName(library.kind)}</p>
      <ul aria-label="Folders" className="mt-4 flex flex-col gap-1 text-sm wrap-anywhere">
        {library.folders.map((folder) => (
          <li key={folder}>{folder}</li>
        ))}
      </ul>
      {/* Pulled back by the buttons' own padding, so the first one's words line up with the name. */}
      <div className="mt-6 -ms-4 flex flex-wrap items-center gap-1">
        <ScanButton library={library} />
        <DeleteLibrary library={library} />
      </div>
      <div className="mt-2 max-w-sm">
        <ScanStatus libraryId={library.id} idle="No scan seen yet." />
      </div>
      <section aria-labelledby="edit" className="mt-10 max-w-sm">
        <h2 id="edit" className="font-medium">
          Name and folders
        </h2>
        <EditLibrary library={library} />
      </section>
    </Page>
  );
}
