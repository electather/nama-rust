import { useRef } from "react";

import { DeleteShow } from "@/features/title/components/delete-show";
import { EditImages } from "@/features/title/components/edit-images";
import { MoreActions } from "@/features/title/components/more-actions";
import { ShowMenuItems } from "@/features/title/components/show-menu-items";
import { showImages } from "@/features/title/lib/title-images";
import type { Season, Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { Opener } from "@/shared/lib/opener";

/**
 * A Show's "…" menu, acting on the Show, with the dialogs its items open beside it, so they stay
 * once the menu closes. Edit images also edits the Poster of the Season the row is on, `season`.
 */
export function ShowMoreMenu({
  libraryId,
  showId,
  show,
  season,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
  season: Season | undefined;
}) {
  const imagesOpener = useRef<Opener>(null);
  const deleteOpener = useRef<Opener>(null);
  return (
    <>
      <MoreActions label="More actions">
        <ShowMenuItems {...{ libraryId, showId, imagesOpener, deleteOpener }} />
      </MoreActions>
      <EditImages ref={imagesOpener} title={show.title} {...showImages(showId, show, season)} />
      <DeleteShow {...{ libraryId, showId }} title={show.title} opener={deleteOpener} />
    </>
  );
}
