import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { PlayerError } from "@/app/route-error/player-error";
import { MoviePlayer } from "@/features/player/components/movie-player";
import { versionSearch } from "@/features/player/lib/version-search";
import { loadMovie } from "@/shared/hooks/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/play")({
  validateSearch: versionSearch,
  loader: ({ context, params }) => loadMovie(context, params),
  staticData: { fullScreen: true },
  notFoundComponent: NotFound,
  errorComponent: PlayerError,
  component: PlayMovie,
});

function PlayMovie() {
  return (
    <MoviePlayer
      input={Route.useLoaderData()}
      version={Route.useSearch().version}
      params={Route.useParams()}
    />
  );
}
