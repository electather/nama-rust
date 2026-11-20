import { useRouteContext } from "@tanstack/react-router";

import type { MovieHit } from "@/gen/nama/v1/request_pb";
import { MovieCard } from "@/shared/components/title-cards/movie-card";

/** A Movie of a row missing from the Server, with the signed-in User's part in its Request. */
export function MissingMovieItem({ movie }: { movie: MovieHit }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return <MovieCard movie={movie} you={signedIn.user?.id} />;
}
