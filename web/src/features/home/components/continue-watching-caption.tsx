import { Play } from "lucide-react";

import { TitleOrLogo } from "@/features/home/components/title-or-logo";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { isStarted, watchedPercent } from "@/shared/lib/watch-progress";

/** A card's Title Logo: up to 48px tall and 60% of the card wide (DESIGN.md, "Backdrop cards"). */
const cardLogo = { className: "max-h-12 max-w-3/5", sizes: "264px" };

/**
 * A Continue Watching card's title over its art, unless the art carries it, and under it ▶ and its
 * `detail`: once started, a bar of how far in you are beside what's left. Its link says all this,
 * so it's hidden from assistive technology.
 */
export function ContinueWatchingCaption({
  title,
  logoUrl,
  titled,
  detail,
  state,
}: {
  title: string;
  logoUrl: string | undefined;
  /** Whether the art carries the title, which then isn't drawn over it. */
  titled: boolean;
  detail: string | undefined;
  state: WatchState | undefined;
}) {
  // A sliver even just started, so the bar reads as a bar, as the artboard draws it.
  const played = `${Math.max(8, watchedPercent(state))}%`;
  return (
    <div aria-hidden className="flex min-w-0 flex-1 flex-col gap-1 pb-2.5">
      {!titled && (
        <span className="truncate text-tile font-semibold">
          <TitleOrLogo title={title} logoUrl={logoUrl} logo={cardLogo} />
        </span>
      )}
      <span className="flex items-center gap-2 text-sm">
        <Play className="size-3.5 fill-current" />
        {isStarted(state) && (
          <span
            className="h-1 w-11 flex-none overflow-hidden rounded-full bg-foreground/25 before:block before:h-full before:w-(--played) before:rounded-full before:bg-foreground"
            style={{ "--played": played }}
          />
        )}
        <span className="whitespace-nowrap tabular-nums">{detail}</span>
      </span>
    </div>
  );
}
