import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { Link, type LinkOptions } from "@tanstack/react-router";
import { ArrowLeft, Maximize, Minimize, Pause, Play, Volume2, VolumeX } from "lucide-react";
import { type PointerEvent, type ReactNode, type RefObject, useRef } from "react";

import { PlaybackInfo } from "@/components/playback-info";
import { Button, buttonVariants } from "@/components/ui/button";
import { SubtitleTracks } from "@/components/subtitles-menu";
import { TrackMenus } from "@/components/track-menus";
import { Slider } from "@/components/ui/slider";
// oxlint-disable-next-line import/max-dependencies -- the player puts together the video, its controls, Tracks, Watch State and playback info
import type { Track } from "@/gen/nama/v1/browse_pb";
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
  useShortcuts,
  useVideo,
} from "@/lib/playback";
import { usePlaybackPlan } from "@/lib/playback-plan";
import { usePlaybackSource } from "@/lib/playback-source";
import { useWatchState } from "@/lib/watch-state";

/** What the player plays, and where "back" goes. */
export interface PlayerProps {
  /** The Movie's or Episode's title, which also names the video. */
  title: string;
  versionId: bigint;
  /** The Version's Tracks, whose audio ones the User chooses from. */
  tracks: Track[];
  /** Your Watch State of the Movie or Episode: where playing resumes. */
  watchState: WatchState | undefined;
  /** The title page to go back to. */
  back: LinkOptions;
}

/**
 * Plays a Version full screen with Nama's own controls, from where the Server's Playback Plan says,
 * asking for a new plan when the browser fails to play one, or says so plainly when it can't play
 * here.
 */
export function Player(props: PlayerProps) {
  const planned = usePlaybackPlan(props);
  const { plan } = planned;
  if (plan.url === "") return <CantPlay back={props.back} reasons={plan.reasons} />;
  if (planned.gaveUp) return <CantPlay back={props.back} reasons={[]} />;
  return (
    // A plan at a new URL plays in a new `<video>`, so it starts where that plan does. ponytail: that one
    // starts unmuted at full volume; carry those over if anyone minds.
    <VideoPlayer
      key={plan.url}
      {...props}
      planned={planned}
      playbackInfo={<PlaybackInfo plan={plan} />}
    />
  );
}

// A click pauses or plays; a tap only brings the controls back.
function clickToToggle(event: PointerEvent<HTMLVideoElement>) {
  if (event.pointerType === "mouse") togglePlay(event.currentTarget);
}

function VideoPlayer({
  title,
  versionId,
  tracks,
  back,
  planned,
  playbackInfo,
}: PlayerProps & { planned: ReturnType<typeof usePlaybackPlan>; playbackInfo: ReactNode }) {
  const { plan, startPositionMs, onCantPlay } = planned;
  const playerRef = useRef<HTMLDivElement>(null);
  const { ref: videoRef, events, ...video } = useVideo();
  const src = usePlaybackSource(videoRef, plan, onCantPlay);
  useWatchState(videoRef, versionId, startPositionMs);
  const [awake, wake] = useAwake();
  useShortcuts(videoRef, playerRef, back, wake);

  return (
    <div
      ref={playerRef}
      data-controls-hidden={(video.state.playing && !awake) || undefined}
      onPointerMove={wake}
      onPointerDown={wake}
      className="group fixed inset-0 bg-background data-controls-hidden:cursor-none"
    >
      {/* oxlint-disable-next-line jsx-a11y/media-has-caption -- a Version may have no subtitles. */}
      <video
        ref={videoRef}
        src={src}
        aria-label={title}
        autoPlay
        playsInline
        onError={(event) => {
          onCantPlay(event.currentTarget);
        }}
        onPointerUp={clickToToggle}
        className="size-full object-contain"
        {...events}
      >
        <SubtitleTracks subtitles={plan.subtitles} />
      </video>
      {/* Any key brings the controls back, so a focused control shows its ring when it's used. */}
      <div className="pointer-events-none absolute inset-0 flex flex-col justify-between transition-opacity duration-500 ease-out-expo group-data-controls-hidden:opacity-0 motion-reduce:transition-none">
        <TopBar title={title} back={back} playbackInfo={playbackInfo} />
        <BottomBar video={video} playerRef={playerRef}>
          <TrackMenus tracks={tracks} planned={planned} videoRef={videoRef} container={playerRef} />
        </BottomBar>
      </div>
    </div>
  );
}

function TopBar({
  title,
  back,
  playbackInfo,
}: {
  title: string;
  back: LinkOptions;
  playbackInfo: ReactNode;
}) {
  return (
    <header className="pointer-events-auto flex group-data-controls-hidden:pointer-events-none items-center gap-2 bg-linear-to-b from-scrim px-2 pt-2 pb-10 sm:px-4 sm:pt-4">
      <BackLink back={back} variant="overlay" size="icon" label="Back">
        <ArrowLeft aria-hidden />
      </BackLink>
      <h1 className="me-auto truncate font-medium">{title}</h1>
      {playbackInfo}
    </header>
  );
}

/** The position, Play and the time, then `children` beside the volume and full screen. */
function BottomBar({
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
          {children}
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

/** Says the Version can't play here: why, as its plan's `reasons` say, or else in general. */
function CantPlay({ back, reasons }: { back: LinkOptions; reasons: string[] }) {
  return (
    <main className="mx-auto flex min-h-dvh max-w-md flex-col justify-center px-6">
      <h1 className="text-2xl font-semibold tracking-tight text-balance">
        This Version can’t play here
      </h1>
      {reasons.length > 0 ? (
        <ul className="mt-2 space-y-1 text-muted-foreground">
          {reasons.map((reason) => (
            <li key={reason}>{reason}</li>
          ))}
        </ul>
      ) : (
        <p className="mt-2 text-muted-foreground">
          This browser can’t play it. Another browser, or another Version, may play.
        </p>
      )}
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
