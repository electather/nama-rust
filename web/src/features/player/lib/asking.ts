import { toMs } from "@/features/player/lib/to-ms";
import { PlaybackLimit, type PlaybackMethod, type PlaybackPlan } from "@/gen/nama/v1/playback_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** What the player asks a plan for, beside the Version. */
export interface Asking {
  failedMethods: PlaybackMethod[];
  startPositionMs: bigint;
  /**
   * The chosen audio Track's place in the Version's Tracks; until the player has a plan,
   * `undefined`, for the Server to choose by the User's Audio Preference (ADR 0035).
   */
  audioTrack: number | undefined;
  /**
   * The Playback Limit chosen, `PLAYBACK_LIMIT_UNSPECIFIED` until the User picks one, when the
   * Server may pick its default for remote playback.
   */
  playbackLimit: PlaybackLimit;
  /** The place of the subtitle Track burned into the picture, if any, in the Version's Tracks. */
  subtitleTrack: number | undefined;
  /**
   * Whether none is burned in, as the player says once it has a plan; until then, with no
   * `subtitleTrack`, the Server chooses by the User's Subtitle Preference (ADR 0035).
   */
  noSubtitles: boolean;
  /** What the User chose for this ask, which the plan may drop. */
  justChose: Choice;
  /** The plan this ask replaces, which the Server is told stopped first, if any. */
  replacing: bigint | undefined;
  /**
   * How many times the player asked again, so each plan plays in a new `<video>`: one that keeps
   * the last plan's URL, as Direct Play does when its choice is dropped, would stay paused.
   */
  asks: number;
}

/**
 * A subtitle the User chose: one the Device shows itself by the URL it's fetched from, one to burn
 * in by its place in the Version's Tracks, or `undefined` for none.
 */
export type SubtitleChoice = string | number | undefined;

/** What the User chose, as `Asking` holds it. */
export interface Choice {
  audioTrack?: number | undefined;
  playbackLimit?: PlaybackLimit;
  subtitleTrack?: number | undefined;
  noSubtitles?: boolean;
}

/** The first ask, which starts where `watchState` stopped. */
export function firstAsking(watchState: WatchState | undefined): Asking {
  return {
    failedMethods: [],
    startPositionMs: watchState && !watchState.played ? watchState.positionMs : 0n,
    audioTrack: undefined,
    playbackLimit: PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED,
    subtitleTrack: undefined,
    noSubtitles: false,
    justChose: {},
    replacing: undefined,
    asks: 0,
  };
}

/**
 * The choices `plan` honoured, which the next ask carries in place of those `before` asked for, so
 * a dropped one is forgotten rather than honoured later when something else is chosen. A remote
 * default the User never picked stays unpicked.
 */
export function honoured(plan: PlaybackPlan, before: Asking): Choice {
  const unpicked = before.playbackLimit === PlaybackLimit.PLAYBACK_LIMIT_UNSPECIFIED;
  return {
    audioTrack: plan.audioTrack,
    subtitleTrack: plan.subtitleTrack,
    noSubtitles: plan.subtitleTrack === undefined,
    playbackLimit: unpicked ? before.playbackLimit : plan.playbackLimit,
  };
}

/**
 * Where `video` stands in milliseconds, or `startPositionMs` if it never loaded far enough to have
 * started there.
 */
export function resumeFromMs(video: HTMLVideoElement, startPositionMs: bigint) {
  if (video.readyState < HTMLMediaElement.HAVE_METADATA) {
    return startPositionMs;
  }
  return toMs(video.currentTime);
}
