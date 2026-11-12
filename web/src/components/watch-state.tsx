import { Check, Circle, CircleCheck, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** How far into a Movie or Episode you are, as a percentage: 0 unless started and not played. */
function watchedPercent(state: WatchState | undefined) {
  if (!state || state.played || state.durationMs === 0n) return 0;
  return Number((state.positionMs * 100n) / state.durationMs);
}

/** A bar showing how far in you are, when you've started and not played it. */
export function WatchedBar({ state }: { state: WatchState | undefined }) {
  const percent = watchedPercent(state);
  if (percent === 0) return null;
  return <Progress value={percent} aria-label="Watched" />;
}

/** ▶ and a bar showing how far in you are, unless it's played or you haven't started. */
export function ResumeBar({ state }: { state: WatchState | undefined }) {
  if (!state || state.played || state.positionMs === 0n) return null;
  return (
    <div className="mt-2 flex items-center gap-2 text-foreground">
      <Play aria-hidden className="size-3.5 shrink-0 fill-current" />
      {/* Not `WatchedBar`, which hides under 1%: in Continue Watching, started is started. */}
      <Progress value={watchedPercent(state)} aria-label="Watched" />
    </div>
  );
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
    <Button
      variant="ghost"
      size="icon"
      aria-label={label}
      aria-pressed={played}
      onClick={() => {
        onToggle(!played);
      }}
    >
      <PlayedIcon played={played} />
    </Button>
  );
}
