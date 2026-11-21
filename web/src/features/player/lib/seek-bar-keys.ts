import { type SeekSection, nextStart, previousStart } from "@/features/player/lib/seek-sections";
import { seekBy } from "@/features/player/lib/video-actions";

/** How far the arrows seek, as YouTube's do; J and L seek further. */
const ARROW_SEEK_SECONDS = 5;

/** What goes with Left and Right to go to a section's start: ⌥ on a Mac, as YouTube's, else Ctrl. */
const SECTION_MODIFIER = /Mac|iPhone|iPad/u.test(globalThis.navigator.platform)
  ? "altKey"
  : "ctrlKey";

const ARROW = /^Arrow(?:Left|Right)$/u;

/** A shortcut that seeks, which needs only the video, and the sections to go to. */
export type SeekShortcut = (target: { video: HTMLVideoElement; sections: SeekSection[] }) => void;

/** Seeks `seconds` forward, or back if negative. */
export const seek =
  (seconds: number): SeekShortcut =>
  ({ video }) => {
    seekBy(video, seconds);
  };

/** Seeks to the start of a section of `sections`, if `to` finds one. */
function toSection(
  to: (sections: SeekSection[], time: number) => number | undefined,
): SeekShortcut {
  return ({ video, sections }) => {
    const start = to(sections, video.currentTime);
    if (start !== undefined) {
      video.currentTime = start;
    }
  };
}

/** The keys that seek by a step or to a section, which the focused seek bar also hands over. */
export const seekBarShortcuts: Record<string, SeekShortcut> = {
  ArrowLeft: seek(-ARROW_SEEK_SECONDS),
  ArrowRight: seek(ARROW_SEEK_SECONDS),
  SectionArrowLeft: toSection(previousStart),
  SectionArrowRight: toSection(nextStart),
};

/** Whether Ctrl, ⌘ or Alt is held, which make most keys the browser's. */
export function hasModifier(event: KeyboardEvent) {
  return event.ctrlKey || event.metaKey || event.altKey;
}

/** Left or Right with the section modifier alone, so Ctrl+Alt+Left and the like stay the system's. */
export function isSectionKey(event: KeyboardEvent) {
  const alone = Number(event.ctrlKey) + Number(event.altKey) + Number(event.metaKey) === 1;
  return alone && event[SECTION_MODIFIER] && ARROW.test(event.key);
}

/**
 * What `event`'s key does on the focused seek bar, which hands the player Left and Right, alone
 * (5 s rather than the slider's 1) or with Ctrl/⌥ (to a section), and keeps its other keys.
 */
export function seekBarShortcutFor(event: KeyboardEvent) {
  const key = handedKey(event);
  return key === undefined ? undefined : seekBarShortcuts[key];
}

function handedKey(event: KeyboardEvent) {
  if (isSectionKey(event)) {
    return `Section${event.key}`;
  }
  return isPlain(event) && ARROW.test(event.key) ? event.key : undefined;
}

function isPlain(event: KeyboardEvent) {
  return !hasModifier(event) && !event.shiftKey;
}
