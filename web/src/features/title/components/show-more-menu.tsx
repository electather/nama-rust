import { useRef } from "react";

import { DeleteShow } from "@/features/title/components/delete-show";
import { EditImages } from "@/features/title/components/edit-images";
import { FixMatch } from "@/features/title/components/fix-match";
import { MoreActions } from "@/features/title/components/more-actions";
import { ServerItems } from "@/features/title/components/server-items";
import { ShowMenuItems } from "@/features/title/components/show-menu-items";
import type { PlayFrom } from "@/features/title/lib/server-choices";
import { showImages } from "@/features/title/lib/title-images";
import { titleToMatch } from "@/features/title/lib/title-to-match";
import type { Season, Show } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { Opener } from "@/shared/lib/opener";

/**
 * A Show's "…" menu, acting on the Show, with the dialogs its items open beside it, so they stay
 * once the menu closes, and at the Hub the choice of which Server Play plays from, `playFrom`.
 * Edit images also edits the Poster of the Season the row is on, `season`.
 */
export function ShowMoreMenu({
  libraryId,
  showId,
  show,
  season,
  playFrom,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
  season: Season | undefined;
  playFrom: PlayFrom;
}) {
  const images = useRef<Opener>(null);
  const fix = useRef<Opener>(null);
  const unmatch = useRef<Opener>(null);
  const remove = useRef<Opener>(null);
  const toMatch = titleToMatch({ libraryId, id: showId }, TitleKind.SHOW, {
    title: show.title,
    date: show.firstAirDate,
  });
  return (
    <>
      <MoreActions label="More actions">
        <ServerItems {...playFrom} />
        <ShowMenuItems {...{ libraryId, showId }} openers={{ images, fix, unmatch, remove }} />
      </MoreActions>
      <EditImages ref={images} title={show.title} {...showImages(showId, show, season)} />
      <FixMatch fix={fix} unmatch={unmatch} title={toMatch} />
      <DeleteShow {...{ libraryId, showId }} title={show.title} opener={remove} />
    </>
  );
}
