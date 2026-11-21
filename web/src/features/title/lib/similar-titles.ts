import type { LinkOptions } from "@tanstack/react-router";

import type { MovieHit, ShowHit } from "@/gen/nama/v1/request_pb";
import { titleParam } from "@/shared/lib/title-param";

/** A Similar title as a title page's row shows it: a poster that opens it in its Library. */
export interface SimilarTitle {
  key: string;
  title: string;
  year: string | undefined;
  posterUrl: string | undefined;
  posterThumbhash: Uint8Array | undefined;
  link: LinkOptions;
}

/** The Similar titles `hits`, Movies, each opening in the Library the Server found it in. */
export function similarMovies(hits: MovieHit[]): SimilarTitle[] {
  return hits.map((hit) => {
    const params = { libraryId: String(hit.libraryId), movieId: titleParam(hit.id) };
    return { ...art(hit), link: { to: "/libraries/$libraryId/movies/$movieId", params } };
  });
}

/** As `similarMovies`, for Shows. */
export function similarShows(hits: ShowHit[]): SimilarTitle[] {
  return hits.map((hit) => {
    const params = { libraryId: String(hit.libraryId), showId: titleParam(hit.id) };
    return { ...art(hit), link: { to: "/libraries/$libraryId/shows/$showId", params } };
  });
}

/** What `hit`'s poster shows, and the key it is listed under. */
function art(hit: MovieHit | ShowHit) {
  return {
    key: `${hit.libraryId}/${titleParam(hit.id)}`,
    title: hit.title,
    year: hit.year?.toString(),
    posterUrl: hit.posterUrl,
    posterThumbhash: hit.posterThumbhash,
  };
}
