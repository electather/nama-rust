import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { Link, type LinkOptions } from "@tanstack/react-router";
import { ArrowLeft, Maximize, Minimize, Pause, Play, Volume2, VolumeX } from "lucide-react";
import { type ReactNode, type RefObject, useRef, useState } from "react";

import { PlaybackInfo } from "@/components/playback-info";
import { Button, buttonVariants } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import {
  type Video,
  clock,
  setVolume,
  toggleFullscreen,
  toggleMute,
  togglePlay,
  useAwake,
  useIsFullscreen,
  usePlaybackPlan,
  useShortcuts,
  useVideo,
} from "@/lib/playback";
// oxlint-disable-next-line import/max-dependencies -- the player puts together the video, its controls, Watch State and playback info
import { useWatchState } from "@/lib/watch-state";

/** What the player plays, and where "back" goes. */
export interface PlayerProps {
  /** The Movie's or Episode's title, which also names the video. */
  title: string;
  versionId: bigint;
  /** Your Watch State of the Movie or Episode: where playing resumes. */
  watchState: WatchState | undefined;
  /** The title page to go back to. */
  back: LinkOptions;
}

/**
 * Plays a Version full screen with Nama's own controls, from where the Server's Playback Plan says,
 * or says so plainly when it can't play here.
 */
export function Player(props: PlayerProps) {
  const plan = usePlaybackPlan(props.versionId, props.watchState);
  const [cantPlay, setCantPlay] = useState(false);
  if (cantPlay || plan === undefined || plan.url === "") return <CantPlay back={props.back} />;
  return (
    <VideoPlayer
      {...props}
      url={plan.url}
      info={<PlaybackInfo plan={plan} />}
      onCantPlay={() => {
        setCantPlay(true);
      }}
    />
  );
}

function VideoPlayer({
  title,
  versionId,
  watchState,
  back,
  url,
  info,
  onCantPlay,
}: PlayerProps & { url: string; info: ReactNode; onCantPlay: () => void }) {
  const playerRef = useRef<HTMLDivElement>(null);
  const { ref: videoRef, events, ...video } = useVideo();
  useWatchState(videoRef, versionId, watchState);
  const [awake, wake] = useAwake();
  useShortcuts(videoRef, playerRef, back, wake);
  const controlsHidden = video.state.playing && !awake;

  return (
    <div
      ref={playerRef}
      data-controls-hidden={controlsHidden || undefined}
      onPointerMove={wake}
      onPointerDown={wake}
      className="group fixed inset-0 bg-background data-controls-hidden:cursor-none"
    >
      {/* oxlint-disable-next-line jsx-a11y/media-has-caption -- Versions carry no caption files yet. */}
      <video
        ref={videoRef}
        src={url}
        aria-label={title}
        autoPlay
        playsInline
        onError={onCantPlay}
        onPointerUp={(event) => {
          // A click pauses or plays; a tap only brings the controls back.
          if (event.pointerType === "mouse") togglePlay(event.currentTarget);
        }}
        className="size-full object-contain"
        {...events}
      />
      {/* Any key brings the controls back, so a focused control shows its ring when it's used. */}
      <div className="pointer-events-none absolute inset-0 flex flex-col justify-between transition-opacity duration-500 ease-out-expo group-data-controls-hidden:opacity-0 motion-reduce:transition-none">
        <TopBar title={title} back={back} info={info} />
        <BottomBar video={video} playerRef={playerRef} />
      </div>
    </div>
  );
}

function TopBar({ title, back, info }: { title: string; back: LinkOptions; info: ReactNode }) {
  return (
    <header className="pointer-events-auto flex group-data-controls-hidden:pointer-events-none items-center gap-2 bg-linear-to-b from-scrim px-2 pt-2 pb-10 sm:px-4 sm:pt-4">
      <BackLink back={back} variant="overlay" size="icon" label="Back">
        <ArrowLeft aria-hidden />
      </BackLink>
      <h1 className="truncate font-medium">{title}</h1>
      {info}
    </header>
  );
}

function BottomBar({
  video,
  playerRef,
}: {
  video: Video;
  playerRef: RefObject<HTMLDivElement | null>;
}) {
  const { playing, time, duration } = video.state;
  return (
    <div className="pointer-events-auto bg-linear-to-t group-data-controls-hidden:pointer-events-none from-scrim px-2 pt-10 pb-2 sm:px-4 sm:pb-4">
      <div className="px-2">
        <Slider
          progress
          label="Position"
          valueText={`${clock(time)} of ${clock(duration)}`}
          value={time}
          max={duration}
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
          <VolumeControls video={video} />
          <FullscreenButton playerRef={playerRef} />
        </div>
      </div>
    </div>
  );
}

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

function FullscreenButton({ playerRef }: { playerRef: RefObject<HTMLDivElement | null> }) {
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

function CantPlay({ back }: { back: LinkOptions }) {
  return (
    <main className="mx-auto flex min-h-dvh max-w-md flex-col justify-center px-6">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">
        This Version can’t play here
      </h1>
      <p className="mt-2 text-muted-foreground">
        This browser can’t play it. Another browser, or another Version, may play.
      </p>
      <div className="mt-6 -ms-4">
        <BackLink back={back} variant="ghost">
          Back
        </BackLink>
      </div>
    </main>
  );
}

function BackLink({
  back,
  label,
  children,
  ...variantProps
}: VariantProps<typeof buttonVariants> & {
  back: LinkOptions;
  label?: string;
  children: ReactNode;
}) {
  return (
    <Link {...back} aria-label={label} className={cn(buttonVariants(variantProps))}>
      {children}
    </Link>
  );
}
