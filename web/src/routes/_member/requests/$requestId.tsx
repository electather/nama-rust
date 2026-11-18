import { createFileRoute } from "@tanstack/react-router";

import { Removed } from "@/features/requests/components/removed";
import { RequestPage } from "@/features/requests/components/request-page";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { queryOrNotFound, wholeNumberFromParam } from "@/lib/browse";

export const Route = createFileRoute("/_member/requests/$requestId")({
  loader: async ({ context, params }) => {
    const input = { id: BigInt(wholeNumberFromParam(params.requestId)) };
    await queryOrNotFound(context, RequestService.method.getRequest, input);
    return input;
  },
  notFoundComponent: Removed,
  component: RequestRoute,
});

function RequestRoute() {
  const { id } = Route.useLoaderData();
  return <RequestPage id={id} />;
}
