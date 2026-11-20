import { createFileRoute, notFound, redirect } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { MovieToRequest } from "@/features/title/components/movie-to-request";
import { TitlePending } from "@/features/title/components/title-pending";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { externalIdFromParam, queryOrNotFound } from "@/shared/hooks/browse";
import { may } from "@/shared/lib/permissions";
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
  pendingComponent: TitlePending,
  notFoundComponent: NotFound,
  component: Movie,
});

function Movie() {
  const { movieId } = Route.useLoaderData();
  const { signedIn } = Route.useRouteContext();
  return <MovieToRequest movieId={movieId} you={signedIn.user?.id} />;
}
