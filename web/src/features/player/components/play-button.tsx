import { Pause, Play } from "lucide-react";

import type { Video } from "@/features/player/hooks/use-video";
import { togglePlay } from "@/features/player/lib/video-actions";
import { Button } from "@/shared/ui/button";

/** Plays the video, or pauses it. */
export function PlayButton({ video }: { video: Video }) {
  const { playing } = video.state;
  return (
    <Button
      variant="overlay"
      size="icon"
      aria-label={playing ? "Pause" : "Play"}
      onClick={() => {
        video.control(togglePlay);
      }}
    >
      {playing ? <Pause aria-hidden /> : <Play aria-hidden />}
    </Button>
  );
}
