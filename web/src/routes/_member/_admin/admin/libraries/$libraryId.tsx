import { createFileRoute, notFound } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { LibraryPage } from "@/features/admin-libraries/components/library-page";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { queryOrNotFound } from "@/lib/browse";

/** The Library in the URL's id, or "not found" for one that isn't a number. */
function libraryInput(libraryId: string) {
  if (!/^\d+$/u.test(libraryId)) {
    throw notFound();
  }
  return { id: BigInt(libraryId) };
}

export const Route = createFileRoute("/_member/_admin/admin/libraries/$libraryId")({
  loader: ({ context, params }) =>
    queryOrNotFound(context, LibraryService.method.getLibrary, libraryInput(params.libraryId)),
  notFoundComponent: NotFound,
  component: LibraryRoute,
});

function LibraryRoute() {
  const { libraryId } = Route.useParams();
  return <LibraryPage id={libraryInput(libraryId).id} />;
}
