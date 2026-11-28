import { Titles } from "@/features/library/components/titles";
import { firstPageOf } from "@/features/library/lib/title-listing";
import type { TitlesProps } from "@/features/library/lib/titles-props";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { usePaged } from "@/shared/hooks/use-paged";
import { posterOf } from "@/shared/lib/server-image";
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
        <Poster
          size="tile"
          link={{
            to: "/libraries/$libraryId/movies/$movieId",
            params: { libraryId: String(libraryId), movieId: titleParam(movie.id) },
          }}
          title={movie.title}
          detail={yearOf(movie.releaseDate)}
          art={posterOf(movie)}
        />
      ),
    }));
  return (
    <Titles
      titles={{ one: "Movie", many: "Movies" }}
      index={paged.data.pages[0]?.index ?? []}
      {...{ libraryId, listing, change, paged, posters }}
    />
  );
}
