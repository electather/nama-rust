import type { MessageInitShape } from "@bufbuild/protobuf";

import type { SetPlayedRequestSchema, WatchState } from "@/gen/nama/v1/watch_state_pb";
import { useSetPlayed } from "@/shared/hooks/use-set-played";
import { DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** Marks `target`, a Movie or an Episode of `libraryId`, watched, or unwatched once it's played. */
export function MarkPlayedItem({
  libraryId,
  target,
  watchState,
}: {
  libraryId: bigint;
  target: NonNullable<MessageInitShape<typeof SetPlayedRequestSchema>["target"]>;
  watchState: WatchState | undefined;
}) {
  const setPlayed = useSetPlayed();
  const played = watchState?.played === true;
  return (
    <DropdownMenuItem
      onClick={() => {
        setPlayed({ libraryId, target, played: !played });
      }}
    >
      {played ? "Mark as unwatched" : "Mark as watched"}
    </DropdownMenuItem>
  );
}
