import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { NotFound } from "@/components/route-error";
import { SeasonPicker } from "@/components/season-picker";
import { TitlePage } from "@/components/title-page";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { externalIdFromParam, queryOrNotFound } from "@/lib/browse";
import { may } from "@/lib/permissions";

/** A Show from the metadata source, for picking its Seasons to Request. */
export const Route = createFileRoute("/_member/shows/$showId")({
  beforeLoad: ({ context }) => {
    // Only search results link here, and only for those who may Request Shows.
    if (!may(context.signedIn.user, Permission.REQUEST_TV)) throw notFound();
  },
  loader: async ({ context, params }) => {
    const { id } = externalIdFromParam(params.showId);
    // Requests name Shows by TMDB id.
    if (id.case !== "tmdb") throw notFound();
    const input = { showTmdbId: id.value };
    await queryOrNotFound(context, RequestService.method.lookUpShow, input);
    return input;
  },
  notFoundComponent: NotFound,
  component: ShowToRequest,
});

function ShowToRequest() {
  const input = Route.useLoaderData();
  const { signedIn } = Route.useRouteContext();
  const { show } = useSuspenseQuery(RequestService.method.lookUpShow, input).data;
  if (!show) return <NotFound />;
  return (
    <TitlePage title={show.title} backdropUrl={show.backdropUrl}>
      <p className="text-sm text-muted-foreground">{show.year}</p>
      {show.overview !== undefined && (
        <p className="mt-6 max-w-prose text-pretty">{show.overview}</p>
      )}
      <SeasonPicker show={show} you={signedIn.user?.id} />
    </TitlePage>
  );
}
