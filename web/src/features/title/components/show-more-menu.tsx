import { useRef } from "react";

import { DeleteFromShow } from "@/features/title/components/delete-from-show";
import { EditImages } from "@/features/title/components/edit-images";
import { MoreActions } from "@/features/title/components/more-actions";
import { ShowMenuItems } from "@/features/title/components/show-menu-items";
import { showImages } from "@/features/title/lib/title-images";
import type { Season, Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { Opener } from "@/shared/lib/opener";

/**
 * A Show's "…" menu, acting on the Show or the Season the row is on, `seasonNumber`, with the
 * dialogs its items open beside it, so they stay once the menu closes.
 */
export function ShowMoreMenu({
  libraryId,
  showId,
  show,
  seasons,
  seasonNumber,
  seasonPlayed,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
  seasons: Season[];
  seasonNumber: number;
  seasonPlayed: boolean;
}) {
  const imagesOpener = useRef<Opener>(null);
  const seasonOpener = useRef<Opener>(null);
  const showOpener = useRef<Opener>(null);
  const season = seasons.find(({ number }) => number === seasonNumber);
  return (
    <>
      <MoreActions label="More actions">
        <ShowMenuItems
          {...{ libraryId, showId, seasonNumber, seasonPlayed }}
          {...{ imagesOpener, seasonOpener, showOpener }}
        />
      </MoreActions>
      <EditImages ref={imagesOpener} title={show.title} {...showImages(showId, show, season)} />
      <DeleteFromShow
        {...{ libraryId, showId, seasonOpener, showOpener }}
        title={show.title}
        seasons={seasons.map(({ number }) => number)}
        current={seasonNumber}
      />
    </>
  );
}
