import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { BoxSetPage } from "@/features/title/components/box-set-page";
import { TitlePending } from "@/features/title/components/title-pending";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { externalIdFromParam, queryOrNotFound } from "@/shared/hooks/browse";

/** A Box Set, by its External ID, from a Movie's page or a Movies Library's Box Sets. */
export const Route = createFileRoute("/_member/box-sets/$boxSetId")({
  loader: async ({ context, params }) => {
    const input = { boxSetId: externalIdFromParam(params.boxSetId) };
    await queryOrNotFound(context, BrowseService.method.getBoxSet, input);
    return input;
  },
  pendingComponent: TitlePending,
  notFoundComponent: NotFound,
  component: BoxSet,
});

function BoxSet() {
  const { boxSetId } = Route.useLoaderData();
  return <BoxSetPage boxSetId={boxSetId} />;
}
