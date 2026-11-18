import { Volume2, VolumeX } from "lucide-react";

import type { Video } from "@/features/player/hooks/use-video";
import { setVolume, toggleMute } from "@/features/player/lib/video-actions";
import { Button } from "@/shared/ui/button";
import { Slider } from "@/shared/ui/slider";

/** Mute, and the volume beside it on screens wider than a phone's. */
export function VolumeControls({ video }: { video: Video }) {
  const { volume, muted } = video.state;
  const heard = muted ? 0 : volume;
  return (
    <>
      <Button
        variant="overlay"
        size="icon"
        aria-label={muted ? "Unmute" : "Mute"}
        onClick={() => {
          video.control(toggleMute);
        }}
      >
        {heard === 0 ? <VolumeX aria-hidden /> : <Volume2 aria-hidden />}
      </Button>
      {/* Phones set the volume with their own buttons. */}
      <div className="hidden w-24 pe-2 sm:block">
        <Slider
          label="Volume"
          valueText={`${Math.round(heard * 100)}%`}
          value={heard}
          max={1}
          step={0.05}
          onValueChange={(level) => {
            video.control((element) => {
              setVolume(element, level);
            });
          }}
        />
      </div>
    </>
  );
}
