import { Check, Circle, CircleCheck } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** How far into a Movie or Episode you are, as a percentage: 0 unless started and not played. */
function watchedPercent(state: WatchState | undefined) {
  if (!state || state.played || state.durationMs === 0n) return 0;
  return Number((state.positionMs * 100n) / state.durationMs);
}

/** A bar showing how far in you are, when you've started and not played it. */
export function WatchProgress({ state }: { state: WatchState | undefined }) {
  const percent = watchedPercent(state);
  if (percent === 0) return null;
  return <Progress value={percent} aria-label="Watched" />;
}

/** The mark in a poster's corner once it's played: a green check, named for screen readers. */
export function PlayedMark({ state }: { state: WatchState | undefined }) {
  if (state?.played !== true) return null;
  return (
    <span className="absolute end-2 top-2 inline-flex size-7 items-center justify-center rounded-full bg-scrim text-primary">
      <Check aria-hidden className="size-4" strokeWidth={3} />
      <span className="sr-only">Played</span>
    </span>
  );
}

/** A ticked circle once played, an empty one before: never colour alone. */
function PlayedIcon({ played }: { played: boolean }) {
  return played ? (
    <CircleCheck aria-hidden className="size-5 text-primary" />
  ) : (
    <Circle aria-hidden className="size-5" />
  );
}

/** Whether a Movie is played, as a toggle button that marks it so. */
export function PlayedButton({
  played,
  onToggle,
}: {
  played: boolean;
  onToggle: (played: boolean) => void;
}) {
  return (
    <Button
      variant="ghost"
      aria-pressed={played}
      onClick={() => {
        onToggle(!played);
      }}
    >
      <PlayedIcon played={played} />
      Played
    </Button>
  );
}

/** Whether an Episode is played, as a round icon toggle that marks it so. */
export function PlayedIconButton({
  played,
  label,
  onToggle,
}: {
  played: boolean;
  label: string;
  onToggle: (played: boolean) => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={played}
      onClick={() => {
        onToggle(!played);
      }}
      className="inline-flex size-11 shrink-0 items-center justify-center rounded-full text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <PlayedIcon played={played} />
    </button>
  );
}
