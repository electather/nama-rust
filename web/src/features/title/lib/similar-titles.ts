import type { LinkOptions } from "@tanstack/react-router";

import type { MovieHit, ShowHit } from "@/gen/nama/v1/request_pb";
import type { PosterData } from "@/shared/components/poster/poster";
import { posterOf } from "@/shared/lib/server-image";
import { titleParam } from "@/shared/lib/title-param";

/** A Similar title as a title page's row shows it: a poster that opens it in its Library. */
export type SimilarTitle = PosterData & { key: string };

/** What labels a title besides its name, from its year. */
export type Detail = (year: string | undefined) => string | undefined;

/**
 * The Similar titles `hits`, Movies, each opening in the Library the Server found it in, labelled
 * with its year unless `detail` gives its label.
 */
export function similarMovies(hits: MovieHit[], detail: Detail = (year) => year) {
  return hits.map((hit) => {
    const params = { libraryId: String(hit.libraryId), movieId: titleParam(hit.id) };
    return similar(hit, { to: "/libraries/$libraryId/movies/$movieId", params }, detail);
  });
}

/** As `similarMovies`, for Shows. */
export function similarShows(hits: ShowHit[], detail: Detail = (year) => year) {
  return hits.map((hit) => {
    const params = { libraryId: String(hit.libraryId), showId: titleParam(hit.id) };
    return similar(hit, { to: "/libraries/$libraryId/shows/$showId", params }, detail);
  });
}

/** `hit` as a poster opening `link`, under the key it is listed by. */
function similar(hit: MovieHit | ShowHit, link: LinkOptions, detail: Detail): SimilarTitle {
  return {
    key: `${hit.libraryId}/${titleParam(hit.id)}`,
    title: hit.title,
    detail: detail(hit.year?.toString()),
    art: posterOf(hit),
    link,
  };
}
