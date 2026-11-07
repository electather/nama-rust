import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { Player } from "@/components/player";
import { NotFound } from "@/components/route-error";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import {
  externalIdFromParam,
  queryOrNotFound,
  versionSearch,
  versionToPlay,
  wholeNumberFromParam,
} from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/play")({
  validateSearch: versionSearch,
  loader: async ({ context, params }) => {
    const libraryId = wholeNumberFromParam(params.libraryId);
    const input = { libraryId: BigInt(libraryId), movieId: externalIdFromParam(params.movieId) };
    await queryOrNotFound(context, BrowseService.method.getMovie, input);
    return input;
  },
  staticData: { fullScreen: true },
  notFoundComponent: NotFound,
  component: PlayMovie,
});

function PlayMovie() {
  const { movie } = useSuspenseQuery(BrowseService.method.getMovie, Route.useLoaderData()).data;
  const { version } = Route.useSearch();
  const { libraryId, movieId } = Route.useParams();
  // An Event may have removed it since the loader ran.
  if (!movie) return <NotFound />;
  return (
    <Player
      title={movie.title}
      versionId={versionToPlay(movie.versions, version).id}
      titleHref={`/libraries/${libraryId}/movies/${movieId}`}
    />
  );
}
