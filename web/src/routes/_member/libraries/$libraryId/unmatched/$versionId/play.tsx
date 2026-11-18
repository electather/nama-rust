import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { Player } from "@/components/player";
import { NotFound, PlayerError } from "@/components/route-error";
import { parsedAs } from "@/components/unmatched-item";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { queryOrNotFound, wholeNumberFromParam } from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/unmatched/$versionId/play")({
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

/** Plays an Unmatched Item's Version, which keeps no Watch State (ADR 0013). */
function PlayUnmatchedVersion() {
  // A refetch that finds it Matched since keeps what was read, so playing goes on.
  const { item, version } = useSuspenseQuery(
    BrowseService.method.getUnmatchedVersion,
    Route.useLoaderData(),
  ).data;
  const params = Route.useParams();
  if (!item || !version) return <NotFound />;
  return (
    <Player
      title={parsedAs(item)}
      versionId={version.id}
      tracks={version.tracks}
      watchState={undefined}
      back={{ to: "/libraries/$libraryId", params }}
    />
  );
}
