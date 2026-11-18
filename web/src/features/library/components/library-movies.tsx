import { Link } from "@tanstack/react-router";

import { Titles } from "@/features/library/components/titles";
import { firstPageOf } from "@/features/library/lib/title-listing";
import type { TitlesProps } from "@/features/library/lib/titles-props";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { usePaged } from "@/shared/hooks/use-paged";
import { posterLink } from "@/shared/lib/poster-classes";
import { yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A Movies Library's Movies, sorted and filtered as `listing` says. */
export function LibraryMovies({ libraryId, listing, change }: TitlesProps) {
  const paged = usePaged(BrowseService.method.listMovies, {
    libraryId,
    ...firstPageOf(listing, "movies"),
  });
  const posters = paged.data.pages
    .flatMap((page) => page.movies)
    .map((movie) => ({
      key: titleParam(movie.id),
      poster: (
        <Link
          to="/libraries/$libraryId/movies/$movieId"
          params={{ libraryId: String(libraryId), movieId: titleParam(movie.id) }}
          className={posterLink}
        >
          <Poster
            url={movie.posterUrl}
            thumbhash={movie.posterThumbhash}
            title={movie.title}
            detail={yearOf(movie.releaseDate)}
            watchState={movie.watchState}
          />
        </Link>
      ),
    }));
  return (
    <Titles
      titles={{ one: "Movie", many: "Movies" }}
      {...{ libraryId, listing, change, paged, posters }}
    />
  );
}
