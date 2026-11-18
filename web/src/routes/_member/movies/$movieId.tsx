import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound, redirect } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { FoundMovieState } from "@/components/found-movie-state";
import { TitlePage } from "@/components/title-page";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { externalIdFromParam, queryOrNotFound } from "@/lib/browse";
import { may } from "@/shared/lib/permissions";
import { metadataLine, runtimeOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A Movie from the title source, for Requesting it. One a Library has opens there instead. */
export const Route = createFileRoute("/_member/movies/$movieId")({
  beforeLoad: ({ context }) => {
    // Only search results link here, and only for those who may Request.
    if (!may(context.signedIn.user, Permission.REQUEST_MOVIE)) {
      throw notFound();
    }
  },
  loader: async ({ context, params }) => {
    const input = { movieId: externalIdFromParam(params.movieId) };
    const { movie } = await queryOrNotFound(context, RequestService.method.lookUpMovie, input);
    if (movie?.libraryId !== undefined) {
      throw redirect({
        to: "/libraries/$libraryId/movies/$movieId",
        params: { libraryId: String(movie.libraryId), movieId: titleParam(movie.id) },
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
  if (!movie) {
    return <NotFound />;
  }
  return (
    <TitlePage
      title={movie.title}
      backdropUrl={movie.backdropUrl}
      backdropThumbhash={movie.backdropThumbhash}
    >
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
