import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound, redirect } from "@tanstack/react-router";

import { FoundMovieState } from "@/components/request-state";
import { NotFound } from "@/components/route-error";
import { TitlePage } from "@/components/title-page";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { externalIdFromParam, metadataLine, queryOrNotFound, runtimeOf } from "@/lib/browse";

/** A Movie from the metadata source, for Requesting it. One a Library has opens there instead. */
export const Route = createFileRoute("/_member/movies/$movieId")({
  loader: async ({ context, params }) => {
    const { id } = externalIdFromParam(params.movieId);
    // Requests name Movies by TMDB id.
    if (id.case !== "tmdb") throw notFound();
    const input = { movieTmdbId: id.value };
    const { movie } = await queryOrNotFound(context, RequestService.method.lookUpMovie, input);
    if (movie?.libraryId !== undefined) {
      throw redirect({
        to: "/libraries/$libraryId/movies/$movieId",
        params: { libraryId: String(movie.libraryId), movieId: params.movieId },
        replace: true,
      });
    }
    return input;
  },
  notFoundComponent: NotFound,
  component: MovieToRequest,
});

function MovieToRequest() {
  const input = Route.useLoaderData();
  const { signedIn } = Route.useRouteContext();
  const { movie } = useSuspenseQuery(RequestService.method.lookUpMovie, input).data;
  if (!movie) return <NotFound />;
  return (
    <TitlePage title={movie.title} backdropUrl={movie.backdropUrl}>
      <p className="text-sm text-muted-foreground">
        {metadataLine(movie.year?.toString(), runtimeOf(movie.runtimeMinutes))}
      </p>
      <div className="mt-6 flex flex-wrap items-center gap-3">
        <FoundMovieState movie={movie} you={signedIn.user?.id} primary />
      </div>
      {movie.overview !== undefined && (
        <p className="mt-8 max-w-prose text-pretty">{movie.overview}</p>
      )}
    </TitlePage>
  );
}
