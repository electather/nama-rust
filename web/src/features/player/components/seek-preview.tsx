import { PreviewFrame } from "@/features/player/components/preview-frame";
import { type FrameSheets, frameAt } from "@/features/player/lib/preview-frame";
import { clock } from "@/shared/lib/clock";

declare module "react" {
  interface CSSProperties {
    /** Where the seek bar's hover card sits, in pixels from the bar's start edge. */
    "--card-at"?: string;
  }
}

/** Half the card's `w-54`, so it stays inside the bar at either end, as the artboard clamps it. */
const HALF_CARD_PX = 108;

/**
 * The seek bar's hover card, as the Chapters artboard draws it: the frame at `seconds` where the
 * Version has Preview frames, then the `name` of its section, if any, and the time. It sits above
 * the bar at `fraction` of its `width`, kept inside it.
 */
export function SeekPreview({
  fraction,
  width,
  seconds,
  name,
  sheets,
}: {
  fraction: number;
  width: number;
  seconds: number;
  name: string | undefined;
  sheets: FrameSheets | undefined;
}) {
  const at = Math.min(Math.max(fraction * width, HALF_CARD_PX), width - HALF_CARD_PX);
  const frame = sheets && frameAt(sheets, seconds);
  return (
    <div
      style={{ "--card-at": `${at}px` }}
      className="absolute inset-s-(--card-at) bottom-full mb-3 flex w-54 -translate-x-1/2 animate-in flex-col gap-2 rounded-xl border bg-popover p-2 text-popover-foreground shadow-md fade-in-0 rtl:translate-x-1/2"
    >
      {frame && <PreviewFrame crop={frame} />}
      <span className="flex items-baseline justify-between gap-2 px-0.5">
        <span className="min-w-0 truncate text-sm font-semibold">{name}</span>
        <span className="shrink-0 text-xs tabular-nums">{clock(seconds)}</span>
      </span>
    </div>
  );
}
