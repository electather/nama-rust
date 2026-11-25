import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { PlayerError } from "@/app/route-error/player-error";
import { UnmatchedPlayer } from "@/features/player/components/unmatched-player";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { queryOrNotFound, wholeNumberFromParam } from "@/shared/hooks/browse";

export const Route = createFileRoute(
  "/_member/_server/libraries/$libraryId/unmatched/$versionId/play",
)({
  loader: async ({ context, params }) => {
    const input = {
      libraryId: BigInt(wholeNumberFromParam(params.libraryId)),
      versionId: BigInt(wholeNumberFromParam(params.versionId)),
    };
    await queryOrNotFound(context, BrowseService.method.getUnmatchedVersion, input);
    return input;
  },
  staticData: { fullScreen: true },
  notFoundComponent: NotFound,
  errorComponent: PlayerError,
  component: PlayUnmatchedVersion,
});

function PlayUnmatchedVersion() {
  return <UnmatchedPlayer input={Route.useLoaderData()} libraryId={Route.useParams().libraryId} />;
}
