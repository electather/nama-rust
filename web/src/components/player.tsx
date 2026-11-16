import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { Link, type LinkOptions } from "@tanstack/react-router";
import { ArrowLeft, Pause, Play } from "lucide-react";
import {
  type PointerEvent,
  type ReactNode,
  type RefObject,
  Suspense,
  useEffect,
  useRef,
  useState,
} from "react";

import { DroppedChoice } from "@/components/dropped-choice";
import { FullscreenButton, VolumeControls } from "@/components/player-controls";
import { PlaybackInfo } from "@/components/playback-info";
import { Button, buttonVariants } from "@/components/ui/button";
import { SubtitleTracks } from "@/components/subtitles-menu";
// oxlint-disable-next-line import/max-dependencies -- the player puts together the video, its controls, Tracks, Watch State and playback info
import { TrackMenus } from "@/components/track-menus";
import { Slider } from "@/components/ui/slider";
import type { Track } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { type Video, clock, togglePlay, useAwake, useShortcuts, useVideo } from "@/lib/playback";
import { usePlaybackPlan } from "@/lib/playback-plan";
import { usePlaybackSource } from "@/lib/playback-source";
import { wordReasons } from "@/lib/playback-wording";
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
 * here. While the Server works a plan out, which the first time can take minutes, it says so.
 */
export function Player(props: PlayerProps) {
  // Above the plan's owner, so a later plan suspends it mounted and it keeps what it asked for.
  return (
    <Suspense fallback={<GettingReady title={props.title} back={props.back} />}>
      <PlannedPlayer {...props} />
    </Suspense>
  );
}

function PlannedPlayer(props: PlayerProps) {
  const planned = usePlaybackPlan(props);
  const { plan } = planned;
  if (plan.url === "")
    return <CantPlay back={props.back} reasons={wordReasons(plan.reasons).decided} />;
  if (planned.gaveUp) return <CantPlay back={props.back} reasons={[]} />;
  const topBar = (
    <TopBar
      title={props.title}
      back={props.back}
      playbackInfo={<PlaybackInfo plan={plan} />}
      notice={<DroppedChoice justChose={planned.justChose} plan={plan} />}
    />
  );
  return (
    // Each new plan plays in a new `<video>`, so it starts where that plan does. ponytail: that one
    // starts unmuted at full volume; carry those over if anyone minds.
    <VideoPlayer key={planned.asks} {...props} planned={planned} topBar={topBar} />
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
  topBar,
}: PlayerProps & { planned: ReturnType<typeof usePlaybackPlan>; topBar: ReactNode }) {
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
      {/* Over drawn subtitles' canvas (libbitsub's is z-10). Any key shows them, so focus rings do. */}
      <div className="pointer-events-none absolute inset-0 z-20 flex flex-col justify-between transition-opacity duration-500 ease-out-expo group-data-controls-hidden:opacity-0 motion-reduce:transition-none">
        {topBar}
        <BottomBar video={video} playerRef={playerRef}>
          <TrackMenus tracks={tracks} planned={planned} videoRef={videoRef} container={playerRef} />
        </BottomBar>
      </div>
    </div>
  );
}

/** The title bar, with a short `notice` under it. */
function TopBar({
  title,
  back,
  playbackInfo,
  notice,
}: {
  title: string;
  back: LinkOptions;
  playbackInfo?: ReactNode;
  notice?: ReactNode;
}) {
  return (
    <div className="bg-linear-to-b from-scrim pb-10">
      <header className="pointer-events-auto flex group-data-controls-hidden:pointer-events-none items-center gap-2 px-2 pt-2 sm:px-4 sm:pt-4">
        <BackLink back={back} variant="overlay" size="icon" label="Back">
          <ArrowLeft aria-hidden />
        </BackLink>
        <h1 className="me-auto truncate font-medium">{title}</h1>
        {playbackInfo}
      </header>
      {notice}
    </div>
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

// React keeps a Suspense fallback up for at least 300 ms, so without this wait a quick plan, as most
// later ones are, would flash this and have it announced.
const NOTICEABLE_WAIT_MS = 1000;

/** The title bar, and once the wait is noticeable, that the Server is getting the Version ready. */
function GettingReady({ title, back }: Pick<PlayerProps, "title" | "back">) {
  const [noticeable, setNoticeable] = useState(false);
  useEffect(() => {
    const timer = setTimeout(() => {
      setNoticeable(true);
    }, NOTICEABLE_WAIT_MS);
    return () => {
      clearTimeout(timer);
    };
  }, []);
  return (
    <div className="fixed inset-0 flex flex-col bg-background">
      <TopBar title={title} back={back} />
      {noticeable && (
        <output className="m-auto px-6 text-muted-foreground">Getting this ready…</output>
      )}
    </div>
  );
}

/** Says the Version can't play here: why, as the reasons that decided its plan say, or else in general. */
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
