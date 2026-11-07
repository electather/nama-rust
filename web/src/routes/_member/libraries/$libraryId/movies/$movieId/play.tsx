import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { Player } from "@/components/player";
import { NotFound } from "@/components/route-error";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loadMovie, versionSearch, versionToPlay } from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/play")({
  validateSearch: versionSearch,
  loader: ({ context, params }) => loadMovie(context, params),
  staticData: { fullScreen: true },
  notFoundComponent: NotFound,
  component: PlayMovie,
});

function PlayMovie() {
  const { movie } = useSuspenseQuery(BrowseService.method.getMovie, Route.useLoaderData()).data;
  const { version } = Route.useSearch();
  const params = Route.useParams();
  // An Event may have removed it since the loader ran.
  if (!movie) return <NotFound />;
  return (
    <Player
      title={movie.title}
      versionId={versionToPlay(movie.versions, version).id}
      watchState={movie.watchState}
      back={{ to: "/libraries/$libraryId/movies/$movieId", params }}
    />
  );
}
