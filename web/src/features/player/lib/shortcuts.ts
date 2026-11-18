import {
  SEEK_STEP_SECONDS,
  seekBy,
  setVolume,
  toggleFullscreen,
  togglePlay,
} from "@/features/player/lib/video-actions";

const VOLUME_STEP = 0.1;

/** What a keyboard shortcut acts on. */
interface ShortcutTarget {
  video: HTMLVideoElement;
  player: HTMLElement | null;
  /** Goes back to the title page. */
  back: () => void;
}

type Shortcut = (target: ShortcutTarget) => void;

const shortcuts: Record<string, Shortcut> = {
  " ": ({ video }) => {
    togglePlay(video);
  },
  ArrowLeft: ({ video }) => {
    seekBy(video, -SEEK_STEP_SECONDS);
  },
  ArrowRight: ({ video }) => {
    seekBy(video, SEEK_STEP_SECONDS);
  },
  ArrowUp: ({ video }) => {
    setVolume(video, video.volume + VOLUME_STEP);
  },
  ArrowDown: ({ video }) => {
    setVolume(video, video.volume - VOLUME_STEP);
  },
  f: ({ player }) => {
    toggleFullscreen(player);
  },
  F: ({ player }) => {
    toggleFullscreen(player);
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
  ArrowLeft: "input",
  ArrowRight: "input",
  ArrowUp: "input",
  ArrowDown: "input",
};

/** What `event`'s key does in the player, if it's not already taken. */
export function shortcutFor(event: KeyboardEvent) {
  return keyIsTaken(event) ? undefined : shortcuts[event.key];
}

// Ctrl+F, Alt+arrows and the like belong to the browser, every key to an open menu or dialog, and
// a prevented key to whatever already used it, such as Esc closing playback info.
function keyIsTaken(event: KeyboardEvent) {
  return event.defaultPrevented || hasModifier(event) || inMenuOrDialog(event) || focusUses(event);
}

// The dialog is looked for on the page, not around the key's target: a modal `<dialog>` makes the
// player inert, but keys still reach `document`, even with focus left on `<body>` by a click on
// its backdrop.
function inMenuOrDialog({ target }: KeyboardEvent) {
  return (
    document.querySelector("dialog[open]") !== null ||
    (target instanceof Element && target.closest('[role="menu"]') !== null)
  );
}

function hasModifier(event: KeyboardEvent) {
  return event.ctrlKey || event.metaKey || event.altKey;
}

function focusUses({ key, target }: KeyboardEvent) {
  const users = keysFocusUses[key];
  return users !== undefined && target instanceof Element && target.closest(users) !== null;
}
