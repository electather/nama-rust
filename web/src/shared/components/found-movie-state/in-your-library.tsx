import { Link } from "@tanstack/react-router";
import { Library } from "lucide-react";

import type { FoundMovie, MovieHit } from "@/gen/nama/v1/request_pb";
import { StateLabel } from "@/shared/components/state-label";
import { titleParam } from "@/shared/lib/title-param";

/** A link to `movie`'s page in the Library `libraryId`, which has it. */
export function InYourLibrary({
  movie,
  libraryId,
}: {
  movie: MovieHit | FoundMovie;
  libraryId: bigint;
}) {
  return (
    <Link
      to="/libraries/$libraryId/movies/$movieId"
      params={{ libraryId: String(libraryId), movieId: titleParam(movie.id) }}
      aria-label={`${movie.title} in your Library`}
      className="rounded-full outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <StateLabel label="In your Library" Icon={Library} />
    </Link>
  );
}
