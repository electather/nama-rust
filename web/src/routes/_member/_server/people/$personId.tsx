import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { PersonPage } from "@/features/title/components/person-page";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { externalIdFromParam, queryOrNotFound } from "@/shared/hooks/browse";

/** A Person, by their External ID, from a name on a title's page or the action bar. */
export const Route = createFileRoute("/_member/_server/people/$personId")({
  loader: async ({ context, params }) => {
    const input = { personId: externalIdFromParam(params.personId) };
    await queryOrNotFound(context, BrowseService.method.getPerson, input);
    return input;
  },
  notFoundComponent: NotFound,
  component: Person,
});

function Person() {
  const { personId } = Route.useLoaderData();
  return <PersonPage personId={personId} />;
}
