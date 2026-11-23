import { ListVideo } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Popover, PopoverContent, PopoverTrigger } from "nama-ui/components/popover";
import { type RefObject, useState } from "react";

import { EpisodesLoader } from "@/features/player/components/episodes-loader";
import type { InShow } from "@/features/player/lib/in-show";
import { PANEL_GAP } from "@/features/player/lib/panel-place";

/**
 * The Episodes button in the title bar, and the panel under it listing a Season's Episodes, the
 * one playing first, each a link that plays it. `played` is how far into this one the video is.
 */
export function EpisodesPanel({
  inShow,
  container,
  played,
}: {
  inShow: InShow;
  container: RefObject<HTMLDivElement | null>;
  played: number;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger render={<Button variant="secondary" size="tall" />}>
        <ListVideo aria-hidden />
        {/* On a phone the Show's name needs the room, so the button is its icon. */}
        <span className="max-sm:sr-only">Episodes</span>
      </PopoverTrigger>
      <PopoverContent
        width="list"
        container={container}
        side="bottom"
        sideOffset={PANEL_GAP}
        aria-label="Episodes"
      >
        <EpisodesLoader
          inShow={inShow}
          played={played}
          onPick={() => {
            setOpen(false);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
