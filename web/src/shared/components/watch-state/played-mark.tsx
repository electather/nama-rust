import { Check } from "lucide-react";

import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** The mark in a poster's corner once it's played: a `primary` check, named for screen readers. */
export function PlayedMark({ state }: { state: WatchState | undefined }) {
  if (state?.played !== true) {
    return null;
  }
  return (
    <span className="absolute inset-e-2 top-2 inline-flex size-7 items-center justify-center rounded-full bg-scrim text-primary">
      <Check aria-hidden className="size-4" strokeWidth={3} />
      <span className="sr-only">Played</span>
    </span>
  );
}
