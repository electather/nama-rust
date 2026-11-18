import { Play } from "lucide-react";

import { Button } from "@/shared/ui/button";

/** Plays the next Episode now. */
export function PlayNowButton({ onPlayNow }: { onPlayNow: () => void }) {
  return (
    <Button onClick={onPlayNow}>
      <Play aria-hidden className="size-4 fill-current" />
      Play now
    </Button>
  );
}
