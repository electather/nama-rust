import { VolumeIcon } from "@/features/player/components/volume-icon";
import type { Video } from "@/features/player/hooks/use-video";
import { setVolume, toggleMute } from "@/features/player/lib/video-actions";
import { Button } from "@/shared/ui/button";
import { Slider } from "@/shared/ui/slider";

/** Mute, which opens to the volume on hover or focus, on screens wider than a phone's. */
export function VolumeControls({ video }: { video: Video }) {
  const { volume, muted } = video.state;
  const heard = muted ? 0 : volume;
  const label = muted ? "Unmute" : "Mute";
  return (
    <div className="group/volume flex items-center">
      <Button
        variant="overlay"
        size="icon"
        aria-label={label}
        title={label}
        onClick={() => {
          video.control(toggleMute);
        }}
      >
        <VolumeIcon heard={heard} />
      </Button>
      {/* Phones set the volume with their own buttons. */}
      <div className="hidden w-0 overflow-hidden px-0 transition-all duration-150 ease-out-expo group-focus-within/volume:w-24 group-focus-within/volume:px-2 group-hover/volume:w-24 group-hover/volume:px-2 motion-reduce:transition-none sm:block">
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
    </div>
  );
}
