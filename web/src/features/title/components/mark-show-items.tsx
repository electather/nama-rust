import type { MessageInitShape } from "@bufbuild/protobuf";

import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { SetPlayedRequestSchema } from "@/gen/nama/v1/watch_state_pb";
import { useSetPlayed } from "@/shared/hooks/use-set-played";
import { seasonName } from "@/shared/lib/title-facts";
import { DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** What to mark: the Season the row is on, or the whole Show. */
type Target = NonNullable<MessageInitShape<typeof SetPlayedRequestSchema>["target"]>;

/**
 * Marks the Season `seasonNumber`, the one the row is on, watched or, once `seasonPlayed`,
 * unwatched, or the whole Show either way.
 */
export function MarkShowItems({
  libraryId,
  showId,
  seasonNumber,
  seasonPlayed,
}: {
  libraryId: bigint;
  showId: ExternalId;
  seasonNumber: number;
  seasonPlayed: boolean;
}) {
  const setPlayed = useSetPlayed();
  const mark = (target: Target, played: boolean) => () => {
    setPlayed({ libraryId, target, played });
  };
  const season: Target = { case: "season", value: { showId, seasonNumber } };
  const show: Target = { case: "showId", value: showId };
  return (
    <>
      <DropdownMenuItem onClick={mark(season, !seasonPlayed)}>
        Mark {seasonName(seasonNumber)} as {seasonPlayed ? "unwatched" : "watched"}
      </DropdownMenuItem>
      <DropdownMenuItem onClick={mark(show, true)}>Mark Show as watched</DropdownMenuItem>
      <DropdownMenuItem onClick={mark(show, false)}>Mark Show as unwatched</DropdownMenuItem>
    </>
  );
}
