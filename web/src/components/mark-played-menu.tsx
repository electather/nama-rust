import type { MessageInitShape } from "@bufbuild/protobuf";
import { Ellipsis } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { SetPlayedRequestSchema } from "@/gen/nama/v1/watch_state_pb";
import { seasonName } from "@/lib/browse";
import { useSetPlayed } from "@/lib/watch-state";

/** What to mark: the Season shown, or the whole Show. */
type Target = NonNullable<MessageInitShape<typeof SetPlayedRequestSchema>["target"]>;

/**
 * Marks the Season shown, or the whole Show, played or unplayed. Tucked in a menu: a Show's own
 * Watch State isn't known without every Season's Episodes, so it offers both ways.
 */
export function MarkPlayedMenu({
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
    <DropdownMenu>
      <DropdownMenuTrigger
        render={<Button variant="ghost" size="icon" aria-label="More for this Show" />}
      >
        <Ellipsis aria-hidden className="size-5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem onClick={mark(season, !seasonPlayed)}>
          Mark {seasonName(seasonNumber)} {seasonPlayed ? "unplayed" : "played"}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={mark(show, true)}>Mark Show played</DropdownMenuItem>
        <DropdownMenuItem onClick={mark(show, false)}>Mark Show unplayed</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
