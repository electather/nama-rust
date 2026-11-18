import { createFileRoute, notFound } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { ShowToRequest } from "@/features/title/components/show-to-request";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { externalIdFromParam, queryOrNotFound } from "@/lib/browse";
import { may } from "@/shared/lib/permissions";

/** A Show from the title source, for picking its Seasons to Request. */
export const Route = createFileRoute("/_member/shows/$showId")({
  beforeLoad: ({ context }) => {
    // Only search results link here, and only for those who may Request Shows.
    if (!may(context.signedIn.user, Permission.REQUEST_TV)) {
      throw notFound();
    }
  },
  loader: async ({ context, params }) => {
    const input = { showId: externalIdFromParam(params.showId) };
    await queryOrNotFound(context, RequestService.method.lookUpShow, input);
    return input;
  },
  notFoundComponent: NotFound,
  component: Show,
});

function Show() {
  const { showId } = Route.useLoaderData();
  const { signedIn } = Route.useRouteContext();
  return <ShowToRequest showId={showId} you={signedIn.user?.id} />;
}
