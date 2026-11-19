import { SkipIcon } from "@/features/player/components/skip-icon";
import type { Video } from "@/features/player/hooks/use-video";
import { SEEK_STEP_SECONDS, seekBy } from "@/features/player/lib/video-actions";
import { Button } from "@/shared/ui/button";

/** Seeks 10 seconds back, or forward. */
export function SkipButton({ video, forward }: { video: Video; forward: boolean }) {
  const label = `${forward ? "Forward" : "Back"} ${SEEK_STEP_SECONDS} seconds`;
  return (
    <Button
      variant="overlay"
      size="icon"
      aria-label={label}
      title={label}
      onClick={() => {
        video.control((element) => {
          seekBy(element, forward ? SEEK_STEP_SECONDS : -SEEK_STEP_SECONDS);
        });
      }}
    >
      <SkipIcon forward={forward} />
    </Button>
  );
}
