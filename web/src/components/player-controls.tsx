import { Maximize, Minimize, Volume2, VolumeX } from "lucide-react";
import type { RefObject } from "react";

import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import {
  type Video,
  setVolume,
  toggleFullscreen,
  toggleMute,
  useIsFullscreen,
} from "@/lib/playback";

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

/** Makes the player full screen, or not, where the browser can. */
export function FullscreenButton({ playerRef }: { playerRef: RefObject<HTMLDivElement | null> }) {
  const isFullscreen = useIsFullscreen();
  // ponytail: iPhones can't make a page full screen, only the video with its own controls.
  if (!document.fullscreenEnabled) return null;
  return (
    <Button
      variant="overlay"
      size="icon"
      aria-label={isFullscreen ? "Exit full screen" : "Full screen"}
      onClick={() => {
        toggleFullscreen(playerRef.current);
      }}
    >
      {isFullscreen ? <Minimize aria-hidden /> : <Maximize aria-hidden />}
    </Button>
  );
}
