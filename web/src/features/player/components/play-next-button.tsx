import { SkipForward } from "lucide-react";

import { Button } from "@/shared/ui/button";

/** Plays the next Episode now, without waiting out the countdown. */
export function PlayNextButton({ onPlayNext }: { onPlayNext: () => void }) {
  return (
    <Button onClick={onPlayNext}>
      <SkipForward aria-hidden className="size-4 fill-current" />
      Next Episode
    </Button>
  );
}
