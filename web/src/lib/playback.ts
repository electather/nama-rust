import { type LinkOptions, useNavigate } from "@tanstack/react-router";
import {
  type RefObject,
  type SyntheticEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

/** How long the controls stay after the pointer or keyboard was last used, while playing. */
const CONTROLS_LINGER_MS = 3000;
const SEEK_STEP_SECONDS = 10;
const VOLUME_STEP = 0.1;

/** What the controls need of the `<video>`: what it's doing, and a way to act on it. */
export type Video = Pick<ReturnType<typeof useVideo>, "state" | "control">;

/** The `<video>`'s ref, what it's doing, and the handlers that keep that up to date. */
export function useVideo() {
  const ref = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState({
    playing: false,
    time: 0,
    duration: 0,
    volume: 1,
    muted: false,
  });
  const sync = ({ currentTarget: video }: SyntheticEvent<HTMLVideoElement>) => {
    setState({
      playing: !video.paused,
      time: video.currentTime,
      // A stream of unknown length reports `Infinity`.
      duration: Number.isFinite(video.duration) ? video.duration : 0,
      volume: video.volume,
      muted: video.muted,
    });
  };
  const events = {
    onPlay: sync,
    onPause: sync,
    onTimeUpdate: sync,
    onDurationChange: sync,
    onVolumeChange: sync,
  };
  /** Runs `action` on the `<video>`, from an event handler. */
  const control = (action: (video: HTMLVideoElement) => void) => {
    if (ref.current) action(ref.current);
  };
  return { ref, state, events, control };
}

/** Whether the pointer or keyboard was used lately, and `wake` to say it just was. */
export function useAwake() {
  const [awake, setAwake] = useState(true);
  const timer = useRef<number>(undefined);
  const sleepLater = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      setAwake(false);
    }, CONTROLS_LINGER_MS);
  }, []);
  const wake = useCallback(() => {
    setAwake(true);
    sleepLater();
  }, [sleepLater]);
  useEffect(() => {
    sleepLater();
    return () => {
      window.clearTimeout(timer.current);
    };
  }, [sleepLater]);
  return [awake, wake] as const;
}

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
    video.currentTime = Math.max(0, video.currentTime - SEEK_STEP_SECONDS);
  },
  ArrowRight: ({ video }) => {
    video.currentTime += SEEK_STEP_SECONDS;
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
    if (!document.fullscreenElement) back();
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

function shortcutFor(event: KeyboardEvent): Shortcut | undefined {
  // Ctrl+F, Alt+arrows and the like belong to the browser.
  if (hasModifier(event) || focusUses(event)) return undefined;
  return shortcuts[event.key];
}

function hasModifier(event: KeyboardEvent) {
  return event.ctrlKey || event.metaKey || event.altKey;
}

function focusUses({ key, target }: KeyboardEvent) {
  const users = keysFocusUses[key];
  return users !== undefined && target instanceof Element && target.closest(users) !== null;
}

/** Space, the arrows, F for full screen, and Esc to go back to the title. */
export function useShortcuts(
  videoRef: RefObject<HTMLVideoElement | null>,
  playerRef: RefObject<HTMLDivElement | null>,
  back: LinkOptions,
  wake: () => void,
) {
  const navigate = useNavigate();
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      wake();
      const shortcut = shortcutFor(event);
      const video = videoRef.current;
      if (!shortcut || !video) return;
      event.preventDefault();
      shortcut({
        video,
        player: playerRef.current,
        back: () => void navigate(back),
      });
    }
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [videoRef, playerRef, back, wake, navigate]);
}

export function togglePlay(video: HTMLVideoElement) {
  // A refused play, such as autoplay the browser blocked, leaves it paused with Play showing.
  if (video.paused) video.play().catch(() => {});
  else video.pause();
}

export function toggleMute(video: HTMLVideoElement) {
  video.muted = !video.muted;
}

/** Sets the volume, kept between 0 and 1; at 0 the video is muted, so the controls show it. */
export function setVolume(video: HTMLVideoElement, level: number) {
  // Rounded, so that stepping down by 0.1 lands on 0 and not a float's remainder above it.
  video.volume = Math.min(1, Math.max(0, Math.round(level * 100) / 100));
  video.muted = video.volume === 0;
}

export function toggleFullscreen(player: HTMLElement | null) {
  if (document.fullscreenElement) void document.exitFullscreen();
  else void player?.requestFullscreen();
}

export function useIsFullscreen() {
  return useSyncExternalStore(subscribeToFullscreen, () => document.fullscreenElement !== null);
}

function subscribeToFullscreen(onChange: () => void) {
  document.addEventListener("fullscreenchange", onChange);
  return () => {
    document.removeEventListener("fullscreenchange", onChange);
  };
}

/** Seconds as a clock, such as "4:05" or "1:02:03". */
export function clock(seconds: number) {
  const whole = Math.floor(seconds);
  const [hours, minutes, rest] = [
    Math.floor(whole / 3600),
    Math.floor(whole / 60) % 60,
    whole % 60,
  ];
  const mmss = `${String(minutes).padStart(hours > 0 ? 2 : 1, "0")}:${String(rest).padStart(2, "0")}`;
  return hours > 0 ? `${hours}:${mmss}` : mmss;
}
