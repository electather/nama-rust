import { create } from "@bufbuild/protobuf";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";
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

import {
  type PlaybackMethod,
  PlaybackPlanSchema,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";
import { findDeviceDecoding } from "@/lib/device-decoding";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { toMs } from "@/lib/watch-state";

/** How long the controls stay after the pointer or keyboard was last used, while playing. */
const CONTROLS_LINGER_MS = 3000;
const SEEK_STEP_SECONDS = 10;
const VOLUME_STEP = 0.1;

/**
 * The Server's Playback Plan for Version `versionId` on this Device, the position in milliseconds
 * it starts from, at first where `watchState` stopped, and `onCantPlay` for when the browser fails
 * to play it. `onCantPlay` asks for a new plan from where the `<video>` stands, without the method
 * that failed, or else sets `gaveUp`: only once per method, so a Server that plans it again can't
 * make the player ask forever. Each plan is asked for once, and never reused after.
 */
export function usePlaybackPlan({
  versionId,
  watchState,
}: {
  versionId: bigint;
  watchState: WatchState | undefined;
}) {
  const transport = useTransport();
  // Where it stood on opening, until a method fails: a new plan would reload the `<video>`, so the
  // refetches each Watch State report causes mustn't ask again.
  const [{ failedMethods, startPositionMs }, setAsking] = useState(() => ({
    failedMethods: [] as PlaybackMethod[],
    startPositionMs: watchState && !watchState.played ? watchState.positionMs : 0n,
  }));
  const [gaveUp, setGaveUp] = useState(false);
  const plan = useSuspenseQuery({
    queryKey: ["planPlayback", String(versionId), failedMethods, String(startPositionMs)],
    queryFn: async () => {
      const deviceDecoding = await findDeviceDecoding();
      const input = { versionId, startPositionMs, deviceDecoding, failedMethods };
      const response = await callUnaryMethod(transport, PlaybackService.method.planPlayback, input);
      return create(PlaybackPlanSchema, response.plan);
    },
    staleTime: "static",
    gcTime: 0,
  }).data;
  // Stable until the plan changes, so hls.js isn't restarted each time the player renders.
  const onCantPlay = useCallback(
    (video: HTMLVideoElement) => {
      // So a failure that leaves its buffer playing isn't heard behind the loading screen.
      video.pause();
      // Read here: React may run the updater later, when the `<video>` has moved on.
      const resumeFrom = resumeFromMs(video, startPositionMs);
      if (failedMethods.includes(plan.method)) setGaveUp(true);
      // hls.js and the `<video>` may both report one failure: only the first asks again.
      else
        setAsking((asking) =>
          asking.failedMethods.includes(plan.method)
            ? asking
            : {
                failedMethods: [...asking.failedMethods, plan.method],
                startPositionMs: resumeFrom,
              },
        );
    },
    [failedMethods, plan.method, startPositionMs],
  );
  return { plan, startPositionMs, gaveUp, onCantPlay };
}

/**
 * Where `video` stands in milliseconds, or `startPositionMs` if it never loaded far enough to have
 * started there.
 */
function resumeFromMs(video: HTMLVideoElement, startPositionMs: bigint) {
  if (video.readyState < HTMLMediaElement.HAVE_METADATA) return startPositionMs;
  return toMs(video.currentTime);
}

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
  return keyIsTaken(event) ? undefined : shortcuts[event.key];
}

// Ctrl+F, Alt+arrows and the like belong to the browser, every key to an open menu, and a
// prevented key to whatever already used it, such as Esc closing playback info.
function keyIsTaken(event: KeyboardEvent) {
  return event.defaultPrevented || hasModifier(event) || inMenu(event) || focusUses(event);
}

function inMenu({ target }: KeyboardEvent) {
  return target instanceof Element && target.closest('[role="menu"]') !== null;
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
