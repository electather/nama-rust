import {
  hasModifier,
  isSectionKey,
  seek,
  seekBarShortcuts,
} from "@/features/player/lib/seek-bar-keys";
import type { SeekSection } from "@/features/player/lib/seek-sections";
import {
  SEEK_STEP_SECONDS,
  seekBy,
  seekToFraction,
  setVolume,
  stepSpeed,
  toggleFullscreen,
  toggleMute,
  togglePictureInPicture,
  togglePlay,
} from "@/features/player/lib/video-actions";

/** The volume the arrows step, as YouTube's do. */
const VOLUME_STEP = 0.05;

/** What a keyboard shortcut acts on. */
interface ShortcutTarget {
  video: HTMLVideoElement;
  player: HTMLElement | null;
  /** Goes back to the title page. */
  back: () => void;
  next: (() => void) | undefined;
  previous: (() => void) | undefined;
  captions: () => void;
  /** How long one frame shows, in seconds. */
  frame: number;
  /** The seek bar's sections, which Ctrl/⌥ with Left and Right go to the start of. */
  sections: SeekSection[];
}

type Shortcut = (target: ShortcutTarget) => void;

/** Seeks one frame `by` frames forward or back, only while paused, as playing would run past it. */
function stepFrame(by: 1 | -1): Shortcut {
  return ({ video, frame }) => {
    if (video.paused) {
      seekBy(video, by * frame);
    }
  };
}

// 0 to 9 seek to that tenth of the way through, as YouTube's do.
const tenths = Object.fromEntries(
  Array.from({ length: 10 }, (_, tenth): [string, Shortcut] => [
    String(tenth),
    ({ video }) => {
      seekToFraction(video, tenth / 10);
    },
  ]),
);

/** The player's keys, YouTube's own (support.google.com/youtube/answer/7631406), by `event.key`. */
const shortcuts: Record<string, Shortcut> = {
  ...tenths,
  ...seekBarShortcuts,
  " ": ({ video }) => {
    togglePlay(video);
  },
  k: ({ video }) => {
    togglePlay(video);
  },
  j: seek(-SEEK_STEP_SECONDS),
  l: seek(SEEK_STEP_SECONDS),
  Home: ({ video }) => {
    seekToFraction(video, 0);
  },
  End: ({ video }) => {
    seekToFraction(video, 1);
  },
  ",": stepFrame(-1),
  ".": stepFrame(1),
  "<": ({ video }) => {
    stepSpeed(video, -1);
  },
  ">": ({ video }) => {
    stepSpeed(video, 1);
  },
  ArrowUp: ({ video }) => {
    setVolume(video, video.volume + VOLUME_STEP);
  },
  ArrowDown: ({ video }) => {
    setVolume(video, video.volume - VOLUME_STEP);
  },
  m: ({ video }) => {
    toggleMute(video);
  },
  c: ({ captions }) => {
    captions();
  },
  f: ({ player }) => {
    toggleFullscreen(player);
  },
  i: ({ video }) => {
    void togglePictureInPicture(video);
  },
  N: ({ next }) => {
    next?.();
  },
  P: ({ previous }) => {
    previous?.();
  },
  // In full screen, Esc leaves full screen instead.
  Escape: ({ back }) => {
    if (!document.fullscreenElement) {
      back();
    }
  },
};

/** The focused elements that already use a key themselves: Space presses a button, and
 * the arrows move a slider. */
const keysFocusUses: Record<string, string> = {
  " ": "button",
  Home: "input",
  End: "input",
  ArrowLeft: "input",
  ArrowRight: "input",
  ArrowUp: "input",
  ArrowDown: "input",
};

/** What `event`'s key does in the player, if it's not already taken. */
export function shortcutFor(event: KeyboardEvent) {
  return keyIsTaken(event) ? undefined : shortcuts[keyOf(event)];
}

// A letter without Shift by its lower case, so Caps Lock doesn't change what K does, while
// Shift+N stays "N". Ctrl/⌥ with an arrow is its own key, as no other modifier is.
function keyOf(event: KeyboardEvent) {
  const { key, shiftKey } = event;
  if (isSectionKey(event)) {
    return `Section${key}`;
  }
  return key.length === 1 && !shiftKey ? key.toLowerCase() : key;
}

// Ctrl+F, Alt+arrows and the like belong to the browser, but for the section keys; every key in
// an open menu, panel or dialog to it, and a prevented key to whatever already used it.
function keyIsTaken(event: KeyboardEvent) {
  return event.defaultPrevented || forBrowser(event) || inMenuOrDialog(event) || focusUses(event);
}

function forBrowser(event: KeyboardEvent) {
  return hasModifier(event) && !isSectionKey(event);
}

// The dialog or panel is looked for on the page, not around the key's target: a modal `<dialog>` makes the
// player inert, but keys still reach `document`, even with focus left on `<body>` by a click on
// its backdrop.
function inMenuOrDialog({ target }: KeyboardEvent) {
  return (
    document.querySelector('dialog[open], [data-slot="popover-content"]') !== null ||
    (target instanceof Element && target.closest('[role="menu"], [role="dialog"]') !== null)
  );
}

function focusUses({ key, target }: KeyboardEvent) {
  const users = keysFocusUses[key];
  return users !== undefined && target instanceof Element && target.closest(users) !== null;
}
