import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { DeletedWhilePlaying } from "@/app/route-error/deleted-while-playing";
import { NotFound } from "@/app/route-error/not-found";
import { PlayerError } from "@/app/route-error/player-error";
import { Player } from "@/components/player";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { isGone, loadMovie, versionSearch, versionToPlay } from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/play")({
  validateSearch: versionSearch,
  loader: ({ context, params }) => loadMovie(context, params),
  staticData: { fullScreen: true },
  notFoundComponent: NotFound,
  errorComponent: PlayerError,
  component: PlayMovie,
});

function PlayMovie() {
  const { data, error } = useSuspenseQuery(BrowseService.method.getMovie, Route.useLoaderData());
  const { movie } = data;
  const { version } = Route.useSearch();
  const params = Route.useParams();
  // An Event removed it since the loader ran.
  if (!movie || isGone(error)) {
    return <DeletedWhilePlaying />;
  }
  const { id, tracks } = versionToPlay(movie.versions, version);
  return (
    <Player
      title={movie.title}
      versionId={id}
      tracks={tracks}
      watchState={movie.watchState}
      posterUrl={movie.posterUrl}
      back={{ to: "/libraries/$libraryId/movies/$movieId", params }}
    />
  );
}
