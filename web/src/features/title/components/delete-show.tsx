import { useNavigate } from "@tanstack/react-router";
import type { RefObject } from "react";

import { useMayDelete } from "@/features/title/hooks/use-may-delete";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { DeleteMedia } from "@/shared/components/delete-media/delete-media";
import { deletedSearch } from "@/shared/lib/deleted-search";
import type { Opener } from "@/shared/lib/opener";

/** The dialog that deletes the Show `title`, then goes to its Library. */
export function DeleteShow({
  libraryId,
  showId,
  title,
  opener,
}: {
  libraryId: bigint;
  showId: ExternalId;
  title: string;
  opener: RefObject<Opener | null>;
}) {
  const navigate = useNavigate();
  // Without Delete media, the page holds no dialog it can't open.
  if (!useMayDelete()) {
    return null;
  }
  return (
    <DeleteMedia
      ref={opener}
      action="Delete Show"
      title={title}
      versions={[]}
      target={() => ({ target: { case: "show", value: { libraryId, showId } } })}
      onDeletedEveryVersion={async (notTold) => {
        await navigate({
          to: "/libraries/$libraryId",
          params: { libraryId: String(libraryId) },
          search: deletedSearch(title, notTold),
        });
      }}
    />
  );
}
