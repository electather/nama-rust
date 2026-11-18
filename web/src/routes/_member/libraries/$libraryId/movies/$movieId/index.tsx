import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { MoviePage } from "@/features/title/components/movie-page";
import { loadMovie } from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/")({
  loader: ({ context, params }) => loadMovie(context, params),
  notFoundComponent: NotFound,
  component: Movie,
});

function Movie() {
  const { libraryId, movieId } = Route.useLoaderData();
  return <MoviePage libraryId={libraryId} movieId={movieId} params={Route.useParams()} />;
}
