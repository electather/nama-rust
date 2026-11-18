import { type ReactNode, useRef } from "react";

import { NextEpisodeOverlay } from "@/features/player/components/next-episode-overlay";
import { PlayerControls } from "@/features/player/components/player-controls";
import { SubtitleTracks } from "@/features/player/components/subtitle-tracks";
import { useAwake } from "@/features/player/hooks/use-awake";
import { useMediaSession } from "@/features/player/hooks/use-media-session";
import { useNextEpisode } from "@/features/player/hooks/use-next-episode";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { usePlaybackSource } from "@/features/player/hooks/use-playback-source";
import { useShortcuts } from "@/features/player/hooks/use-shortcuts";
import { useVideo } from "@/features/player/hooks/use-video";
import { useWatchState } from "@/features/player/hooks/use-watch-state";
import { clickToToggle } from "@/features/player/lib/click-to-toggle";
import type { PlannedProps } from "@/features/player/lib/player-props";

/** The `<video>` with its controls, Watch State, lock screen and autoplay, for one plan. */
export function VideoPlayer({
  title,
  versionId,
  tracks,
  back,
  artist,
  posterUrl,
  autoplay,
  planned,
  topBar,
}: PlannedProps & { planned: ReturnType<typeof usePlaybackPlan>; topBar: ReactNode }) {
  const { plan, startPositionMs, onCantPlay } = planned;
  const playerRef = useRef<HTMLDivElement>(null);
  const { ref: videoRef, events, ...video } = useVideo();
  const src = usePlaybackSource(videoRef, plan, onCantPlay);
  useWatchState(videoRef, versionId, startPositionMs);
  const [awake, wake] = useAwake();
  const nextEpisode = useNextEpisode(autoplay, video, wake);
  const { onInput, playNext } = nextEpisode;
  useShortcuts({ videoRef, playerRef }, back, onInput);
  useMediaSession(videoRef, { title, artist, posterUrl });

  return (
    <div
      ref={playerRef}
      data-controls-hidden={(video.state.playing && !awake) || undefined}
      onPointerMove={onInput}
      onPointerDown={onInput}
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
        onEnded={playNext}
        className="size-full object-contain"
        {...events}
      >
        <SubtitleTracks subtitles={plan.subtitles} />
      </video>
      <PlayerControls
        topBar={topBar}
        video={video}
        playerRef={playerRef}
        videoRef={videoRef}
        tracks={tracks}
        planned={planned}
      />
      <NextEpisodeOverlay nextEpisode={nextEpisode} back={back} />
    </div>
  );
}
