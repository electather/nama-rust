import { Pause, Play } from "lucide-react";
import { Button } from "nama-ui/components/button";

import type { Video } from "@/features/player/hooks/use-video";
import { togglePlay } from "@/features/player/lib/video-actions";

/** Plays the video, or pauses it. */
export function PlayButton({ video }: { video: Video }) {
  const { playing } = video.state;
  return (
    // Translucent, not the primary fill: Play is transport, and Skip or Next Episode is the screen's one primary action.
    <Button
      variant="secondary"
      size="round"
      aria-label={playing ? "Pause" : "Play"}
      onClick={() => {
        video.control(togglePlay);
      }}
    >
      {playing ? (
        <Pause aria-hidden className="fill-current" />
      ) : (
        <Play aria-hidden className="fill-current" />
      )}
    </Button>
  );
}
