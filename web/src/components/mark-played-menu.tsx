import { Ellipsis } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { seasonName } from "@/lib/browse";
import { useSetPlayed } from "@/lib/watch-state";

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
  const markShow = (played: boolean) => {
    setPlayed({ libraryId, target: { case: "showId", value: showId }, played });
  };
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" aria-label="More for this Show" />}>
        <Ellipsis aria-hidden className="size-5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem
          onClick={() => {
            setPlayed({
              libraryId,
              target: { case: "season", value: { showId, seasonNumber } },
              played: !seasonPlayed,
            });
          }}
        >
          Mark {seasonName(seasonNumber)} {seasonPlayed ? "unplayed" : "played"}
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => {
            markShow(true);
          }}
        >
          Mark Show played
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => {
            markShow(false);
          }}
        >
          Mark Show unplayed
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
