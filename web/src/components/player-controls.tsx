import { Maximize, Minimize, Pause, Play, Volume2, VolumeX } from "lucide-react";
import type { ReactNode, RefObject } from "react";

import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import {
  type Video,
  clock,
  setVolume,
  toggleFullscreen,
  toggleMute,
  togglePlay,
  useIsFullscreen,
} from "@/lib/playback";

/** Mute, and the volume beside it on screens wider than a phone's. */
function VolumeControls({ video }: { video: Video }) {
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
function FullscreenButton({ playerRef }: { playerRef: RefObject<HTMLDivElement | null> }) {
  const isFullscreen = useIsFullscreen();
  // ponytail: iPhones can't make a page full screen, only the video with its own controls.
  if (!document.fullscreenEnabled) {
    return null;
  }
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

/** The position, Play and the time, then `children` beside the volume and full screen. */
export function BottomBar({
  video,
  playerRef,
  children,
}: {
  video: Video;
  playerRef: RefObject<HTMLDivElement | null>;
  children: ReactNode;
}) {
  const { playing, time, duration } = video.state;
  return (
    <div className="pointer-events-auto bg-linear-to-t from-scrim px-2 pt-10 pb-2 group-data-controls-hidden:pointer-events-none sm:px-4 sm:pb-4">
      <div className="px-2">
        <Slider
          progress
          label="Position"
          valueText={`${clock(time)} of ${clock(duration)}`}
          disabled={!duration}
          value={Math.min(time, duration)}
          max={duration || 1}
          onValueChange={(seconds) => {
            video.control((element) => {
              element.currentTime = seconds;
            });
          }}
        />
      </div>
      <div className="flex items-center gap-1">
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
        <span className="ms-2 text-sm tabular-nums">
          {clock(time)} / {clock(duration)}
        </span>
        <div className="ms-auto flex items-center gap-1">
          {children}
          <VolumeControls video={video} />
          <FullscreenButton playerRef={playerRef} />
        </div>
      </div>
    </div>
  );
}
