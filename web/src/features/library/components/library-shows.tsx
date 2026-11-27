import { PosterTile } from "@/features/library/components/poster-tile";
import { Titles } from "@/features/library/components/titles";
import { firstPageOf } from "@/features/library/lib/title-listing";
import type { TitlesProps } from "@/features/library/lib/titles-props";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { usePaged } from "@/shared/hooks/use-paged";
import { yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A Shows Library's Shows, sorted and filtered as `listing` says. */
export function LibraryShows({ libraryId, listing, change }: TitlesProps) {
  const paged = usePaged(BrowseService.method.listShows, {
    libraryId,
    ...firstPageOf(listing, "shows"),
  });
  const posters = paged.data.pages
    .flatMap((page) => page.shows)
    .map((show) => ({
      key: titleParam(show.id),
      poster: (
        <PosterTile
          link={{
            to: "/libraries/$libraryId/shows/$showId",
            params: { libraryId: String(libraryId), showId: titleParam(show.id) },
          }}
          title={show.title}
          detail={yearOf(show.firstAirDate)}
          art={{ url: show.posterUrl, thumbhash: show.posterThumbhash }}
        />
      ),
    }));
  return (
    <Titles
      titles={{ one: "Show", many: "Shows" }}
      index={paged.data.pages[0]?.index ?? []}
      {...{ listing, change, paged, posters }}
    />
  );
}
