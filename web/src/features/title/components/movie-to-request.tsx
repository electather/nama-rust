import { notFound } from "@tanstack/react-router";

import { MovieRequestHero } from "@/features/title/components/movie-request-hero";
import { TitleDetails } from "@/features/title/components/title-details";
import { TitlePage } from "@/features/title/components/title-page";
import { useFoundMovie } from "@/features/title/data/use-found-movie";
import type { ExternalId } from "@/gen/nama/v1/match_pb";

/**
 * A Movie from the title source, for the signed-in User `you` to Request: its top, with Request or
 * where its Request is, then its Details.
 */
export function MovieToRequest({ movieId, you }: { movieId: ExternalId; you: bigint | undefined }) {
  const movie = useFoundMovie({ movieId });
  if (!movie) {
    throw notFound();
  }
  return (
    <TitlePage hero={<MovieRequestHero movie={movie} you={you} />}>
      <TitleDetails
        groups={[{ heading: "Information", facts: [["Released", movie.year?.toString()]] }]}
      />
    </TitlePage>
  );
}
