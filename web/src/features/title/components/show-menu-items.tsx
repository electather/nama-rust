import type { RefObject } from "react";

import { MarkShowItems } from "@/features/title/components/mark-show-items";
import { OpenerItem } from "@/features/title/components/opener-item";
import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";
import type { Opener } from "@/shared/lib/opener";

/**
 * What else may be done to a Show: mark it watched or unwatched, edit its images with Manage
 * Libraries, and delete it with Delete media. `imagesOpener` and `deleteOpener` open those dialogs.
 * Its Seasons' actions are in the "…" beside the Season.
 */
export function ShowMenuItems({
  libraryId,
  showId,
  imagesOpener,
  deleteOpener,
}: {
  libraryId: bigint;
  showId: ExternalId;
  imagesOpener: RefObject<Opener | null>;
  deleteOpener: RefObject<Opener | null>;
}) {
  const mayEditImages = useMay(Permission.MANAGE_LIBRARIES);
  return (
    <>
      <MarkShowItems libraryId={libraryId} showId={showId} />
      {mayEditImages && <OpenerItem opener={imagesOpener}>Edit images</OpenerItem>}
      {useMayDelete() && <OpenerItem opener={deleteOpener}>Delete Show</OpenerItem>}
    </>
  );
}
