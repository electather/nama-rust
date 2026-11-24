import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { MoviePage } from "@/features/title/components/movie-page";
import { TitlePending } from "@/features/title/components/title-pending";
import { loadMovie } from "@/shared/hooks/browse";
import { prefetchLibraries } from "@/shared/hooks/use-libraries";

export const Route = createFileRoute("/_member/libraries/$libraryId/movies/$movieId/")({
  loader: ({ context, params }) => {
    void prefetchLibraries(context);
    return loadMovie(context, params);
  },
  pendingComponent: TitlePending,
  notFoundComponent: NotFound,
  component: Movie,
});

function Movie() {
  const { libraryId, movieId } = Route.useLoaderData();
  return <MoviePage libraryId={libraryId} movieId={movieId} params={Route.useParams()} />;
}
