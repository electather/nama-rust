import { Link, type LinkOptions, useNavigate } from "@tanstack/react-router";
import { Play } from "lucide-react";
import { useCallback, useEffect, useId, useReducer, useRef } from "react";

import { Button, buttonVariants } from "@/components/ui/button";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import { episodeName } from "@/lib/browse";
import { type Video, clock, play } from "@/lib/playback";

const COUNTDOWN_SECONDS = 20;
/** How many Episodes in a row autoplay starts with no input before the player asks instead. */
const ASK_AFTER_STREAK = 3;

/** The Episode that plays after this one, and how to open it. */
export interface NextEpisode {
  episode: Episode;
  /** How many Episodes in a row autoplay started with no input, ending with this one: 0 if it didn't start it. */
  streak: number;
  /** Its play route, opened with `streak`. */
  link: (streak: number) => LinkOptions;
}

/** An Episode player's search param `?streak=`: `NextEpisode.streak`, if autoplay opened it. */
export function streakSearch({ streak }: Record<string, unknown>): { streak?: number } {
  return typeof streak === "number" ? { streak } : {};
}

/**
 * What autoplay keeps of an Episode across its Playback Plans, which each play in a new `<video>`:
 * `next`, and whether the player had input or the countdown was cancelled.
 */
export function useAutoplay(next: NextEpisode | undefined) {
  const [touched, touch] = useReducer(() => true, false);
  const [cancelled, cancel] = useReducer(() => true, false);
  return { next, touched, touch, cancelled, cancel };
}

/** What `useAutoplay` keeps of an Episode. */
export type Autoplay = ReturnType<typeof useAutoplay>;

/**
 * The countdown to the next Episode, and "Are you still watching?" before it, for one `<video>`.
 * The player calls `onInput` on pointer or key input, which also calls `wake`, and `playNext` when
 * the video ends.
 */
export function useNextEpisode(autoplay: Autoplay, video: Video, wake: () => void) {
  const navigate = useNavigate();
  const { next, touched, touch, cancelled } = autoplay;
  const { control, state } = video;
  const due = cancelled ? undefined : dueNext(next, state);
  const asking = isStillWatchingDue(due, touched);
  // Again whenever it starts playing: until it's answered, nothing plays, whatever pressed Play.
  useEffect(() => {
    if (asking && state.playing)
      control((element) => {
        element.pause();
      });
  }, [asking, state.playing, control]);
  // Input while it asks isn't an answer: only Keep watching is.
  const onInput = useCallback(() => {
    wake();
    if (!asking) touch();
  }, [wake, asking, touch]);

  return {
    onInput,
    playNext: () => {
      // The question still waits for an answer if the video ends under it.
      if (next && !cancelled && !asking) void navigate(next.link(streakAfter(next, touched)));
    },
    cancel: autoplay.cancel,
    countdown: asking ? undefined : due,
    secondsLeft: state.duration - state.time,
    asking,
    keepWatching: () => {
      touch();
      control(play);
    },
  };
}

/** `next` while the countdown to it runs, near the end of the video. */
function dueNext(next: NextEpisode | undefined, { time, duration }: Video["state"]) {
  return duration > 0 && duration - time <= COUNTDOWN_SECONDS ? next : undefined;
}

/** The streak autoplay opens `next` with: input during this Episode starts the count again. */
function streakAfter({ streak }: NextEpisode, touched: boolean) {
  return touched ? 1 : streak + 1;
}

function isStillWatchingDue(due: NextEpisode | undefined, touched: boolean) {
  return !touched && due !== undefined && due.streak >= ASK_AFTER_STREAK;
}

/** The countdown to the next Episode, or "Are you still watching?", over the player. */
export function NextEpisodeOverlay({
  nextEpisode,
  back,
}: {
  nextEpisode: ReturnType<typeof useNextEpisode>;
  back: LinkOptions;
}) {
  if (nextEpisode.asking)
    return <StillWatching back={back} onKeepWatching={nextEpisode.keepWatching} />;
  if (!nextEpisode.countdown) return null;
  const {
    countdown: { episode },
    secondsLeft,
  } = nextEpisode;
  return (
    // Outside the controls, so it stays while they're hidden. It floats over the video, so it gets a
    // shadow (DESIGN.md, "Elevation").
    <aside
      aria-label="Next Episode"
      className="absolute inset-x-4 bottom-32 z-30 rounded-xl border bg-popover p-4 text-popover-foreground shadow-lg sm:inset-x-auto sm:end-6 sm:w-80"
    >
      <p className="text-sm text-muted-foreground">
        {episodeName(episode.seasonNumber, episode.episodeNumber, "full")}
      </p>
      <p className="truncate font-medium">{episode.title}</p>
      <p className="mt-1 text-sm text-muted-foreground tabular-nums">
        Starts in {clock(Math.ceil(secondsLeft))}
      </p>
      <div className="mt-4 flex gap-2">
        <Button onClick={nextEpisode.playNext}>
          <Play aria-hidden className="size-4 fill-current" />
          Play now
        </Button>
        <Button variant="ghost" onClick={nextEpisode.cancel}>
          Cancel
        </Button>
      </div>
    </aside>
  );
}

function StillWatching({
  back,
  onKeepWatching,
}: {
  back: LinkOptions;
  onKeepWatching: () => void;
}) {
  const navigate = useNavigate();
  const titleId = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  return (
    // A modal `<dialog>` keeps the pointer and focus off the paused controls until it's answered,
    // and the player's shortcuts leave keys to it.
    <dialog
      ref={dialog}
      role="alertdialog"
      aria-labelledby={titleId}
      // Esc goes Back, as the player's shortcut does, and doesn't just close it.
      onCancel={(event) => {
        event.preventDefault();
        void navigate(back);
      }}
      // Floats above the player, so it gets a shadow (DESIGN.md, "Elevation").
      className="m-auto w-full max-w-xs rounded-xl border bg-popover p-6 text-popover-foreground shadow-xl backdrop:bg-scrim sm:max-w-sm"
    >
      <h2 id={titleId} className="text-lg font-semibold tracking-tight">
        Are you still watching?
      </h2>
      <div className="mt-6 flex gap-2">
        <Button onClick={onKeepWatching}>Keep watching</Button>
        <Link {...back} className={buttonVariants({ variant: "ghost" })}>
          Back
        </Link>
      </div>
    </dialog>
  );
}
