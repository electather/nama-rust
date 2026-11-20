import type { RefObject } from "react";

import { MarkShowItems } from "@/features/title/components/mark-show-items";
import { OpenerItem } from "@/features/title/components/opener-item";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import type { Opener } from "@/shared/lib/opener";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * What else may be done to a Show: mark the Season the row is on, `seasonNumber`, or the Show
 * watched, edit their images with Manage Libraries, and delete either with Delete media.
 * `imagesOpener`, `seasonOpener` and `showOpener` open those dialogs.
 */
export function ShowMenuItems({
  libraryId,
  showId,
  seasonNumber,
  seasonPlayed,
  imagesOpener,
  seasonOpener,
  showOpener,
}: {
  libraryId: bigint;
  showId: ExternalId;
  seasonNumber: number;
  seasonPlayed: boolean;
  imagesOpener: RefObject<Opener | null>;
  seasonOpener: RefObject<Opener | null>;
  showOpener: RefObject<Opener | null>;
}) {
  const mayEditImages = useMay(Permission.MANAGE_LIBRARIES);
  const mayDelete = useMayDelete();
  return (
    <>
      <MarkShowItems {...{ libraryId, showId, seasonNumber, seasonPlayed }} />
      {mayEditImages && <OpenerItem opener={imagesOpener}>Edit images</OpenerItem>}
      {mayDelete && (
        <>
          <OpenerItem opener={seasonOpener}>Delete {seasonName(seasonNumber)}</OpenerItem>
          <OpenerItem opener={showOpener}>Delete Show</OpenerItem>
        </>
      )}
    </>
  );
}
