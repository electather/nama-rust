import type { MessageInitShape } from "@bufbuild/protobuf";
import { Ellipsis } from "lucide-react";
import { useState } from "react";

import type { SetHiddenFromContinueWatchingRequestSchema } from "@/gen/nama/v1/watch_state_pb";
import { useSetHiddenFromContinueWatching } from "@/lib/watch-state";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/** A card's title, keyed as the row lists it, and what hiding it from Continue Watching asks. */
export interface Hiding {
  key: string;
  title: string;
  libraryId: bigint;
  /** A Movie, or a Show by any of its Episodes. */
  target: NonNullable<
    MessageInitShape<typeof SetHiddenFromContinueWatchingRequestSchema>["target"]
  >;
}

/** Hiding titles from Continue Watching: the one `lastHidden`, to `hide` one, and to `undo` it. */
export function useHideFromContinueWatching() {
  const setHidden = useSetHiddenFromContinueWatching();
  const [lastHidden, setLastHidden] = useState<Hiding>();
  const ask = ({ libraryId, target }: Hiding, hidden: boolean, then: () => void) => {
    setHidden({ libraryId, target, hidden }, { onSuccess: then });
  };
  return {
    lastHidden,
    hide: (hiding: Hiding) => {
      ask(hiding, true, () => {
        setLastHidden(hiding);
      });
    },
    undo: (hiding: Hiding) => {
      ask(hiding, false, () => {
        setLastHidden(undefined);
      });
    },
  };
}

/**
 * A card's menu, over the top corner of its art, which stays dark, on the scrim (DESIGN.md, "The
 * Legible Over Art Rule").
 */
export function HideMenu({ title, onHide }: { title: string; onHide: () => void }) {
  return (
    <div className="dark absolute inset-e-2 top-2 rounded-lg bg-scrim">
      <DropdownMenu>
        <DropdownMenuTrigger
          render={<Button variant="overlay" size="icon" aria-label={`More for ${title}`} />}
        >
          <Ellipsis aria-hidden />
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem onClick={onHide}>Hide from Continue Watching</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

/**
 * Says the title `lastHidden` is hidden, with Undo, which takes the focus its card's menu lost. It stays
 * until you leave Home or hide another, rather than timing out on someone still reading it.
 */
export function HiddenNotice({
  lastHidden,
  onUndo,
}: {
  lastHidden: Hiding | undefined;
  onUndo: (lastHidden: Hiding) => void;
}) {
  return (
    // Always there, so a screen reader announces what's put in it.
    <output className="block">
      {lastHidden && (
        <span
          key={lastHidden.key}
          className="mt-2 flex flex-wrap items-center gap-x-2 text-sm text-muted-foreground"
        >
          {lastHidden.title} is hidden until you play it again.
          <Button
            variant="ghost"
            ref={focus}
            onClick={() => {
              onUndo(lastHidden);
            }}
          >
            Undo
          </Button>
        </span>
      )}
    </output>
  );
}

/** A stable ref callback, so React calls it only when the element mounts. */
const focus = (element: HTMLElement | null) => element?.focus();
