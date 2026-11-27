import { PreviewFrame } from "@/features/player/components/preview-frame";
import { SeekCaption } from "@/features/player/components/seek-caption";
import { type FrameCrop, type FrameSheets, frameAt } from "@/features/player/lib/preview-frame";

declare module "react" {
  interface CSSProperties {
    /** Where the seek bar's hover card sits, in pixels from the bar's start edge. */
    "--card-at"?: string;
  }
}

/**
 * The card's shape: `half` is half its width in pixels, so it stays inside the bar at either end,
 * as the artboard clamps it.
 */
const CARD = { half: 108, classes: "w-54 gap-2 rounded-xl p-2" };
/** The time-only card's shape, likewise. */
const TIME_CARD = { half: 36, classes: "w-18 items-center rounded-lg py-1.5" };

/** The card for a `frame` and section `name`: with neither, the time-only one. */
function cardFor(frame: FrameCrop | undefined, name: string | undefined) {
  return frame || name !== undefined ? CARD : TIME_CARD;
}

/**
 * The seek bar's hover card, as the Chapters artboard draws it: the frame at `seconds` where the
 * Version has Preview frames, then the `name` of its section, if any, and the time. With neither a
 * frame nor a name, it shrinks to the time alone. It sits above the bar at `fraction` of its
 * `width`, kept inside it.
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
  const frame = sheets && frameAt(sheets, seconds);
  const card = cardFor(frame, name);
  const at = Math.min(Math.max(fraction * width, card.half), width - card.half);
  return (
    <div
      style={{ "--card-at": `${at}px` }}
      className={`absolute inset-s-(--card-at) bottom-full mb-3 flex -translate-x-1/2 animate-in flex-col border bg-popover text-popover-foreground shadow-md fade-in-0 rtl:translate-x-1/2 ${card.classes}`}
    >
      {frame && <PreviewFrame crop={frame} />}
      <SeekCaption name={name} seconds={seconds} timeOnly={card === TIME_CARD} />
    </div>
  );
}
