import { EpisodeRow } from "@/features/title/components/episode-row";
import { SeasonBar } from "@/features/title/components/season-bar";
import type { ShowRow } from "@/features/title/hooks/use-show-row";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * A Show's `row` of Episodes, `useShowRow`'s, Season after Season, Specials last, under the name of
 * the Season it's on, a menu that scrolls it to another; nothing while it has no Season. Those of
 * the page's Library, `libraryId`, play. The Show's `backdrop` stands in for a missing Still.
 */
export function ShowEpisodes({
  row,
  libraryId,
  backdrop,
}: {
  row: ShowRow;
  libraryId: bigint | undefined;
  backdrop: ServerImage | undefined;
}) {
  const {
    seasons,
    season,
    view: { pick },
  } = row;
  return (
    seasons.length > 0 && (
      <section aria-label="Episodes" className="flex flex-col gap-4">
        <SeasonBar seasons={seasons} current={season} onPick={pick} />
        <EpisodeRow row={row} libraryId={libraryId} look={{ tall: row.tall, backdrop }} />
      </section>
    )
  );
}
