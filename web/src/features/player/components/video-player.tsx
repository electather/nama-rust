import { type ComponentProps, useRef } from "react";

import { BufferingRing } from "@/features/player/components/buffering-ring";
import { FirstFrameCover } from "@/features/player/components/first-frame-cover";
import { NextEpisodeOverlay } from "@/features/player/components/next-episode-overlay";
import { PictureInPictureCover } from "@/features/player/components/picture-in-picture-cover";
import { PlayerControls } from "@/features/player/components/player-controls";
import { SubtitleTracks } from "@/features/player/components/subtitle-tracks";
import type { TopBar } from "@/features/player/components/top-bar";
import { useControlsHidden } from "@/features/player/hooks/use-controls-hidden";
import { useMediaSession } from "@/features/player/hooks/use-media-session";
import { useNextEpisode } from "@/features/player/hooks/use-next-episode";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import { usePlaybackSource } from "@/features/player/hooks/use-playback-source";
import { useShortcuts } from "@/features/player/hooks/use-shortcuts";
import { useVideo } from "@/features/player/hooks/use-video";
import { useWatchState } from "@/features/player/hooks/use-watch-state";
import { clickToToggle } from "@/features/player/lib/click-to-toggle";
import { nextLink } from "@/features/player/lib/in-show";
import { captionsToggled, frameSeconds } from "@/features/player/lib/key-targets";
import type { PlannedProps } from "@/features/player/lib/player-props";

/** The `<video>` with its controls, Watch State, lock screen and autoplay, for one plan. */
export function VideoPlayer({
  title,
  versionId,
  tracks,
  back,
  artist,
  posterUrl,
  backdropUrl,
  autoplay,
  planned,
  topBar,
  inShow,
}: PlannedProps & {
  planned: ReturnType<typeof usePlaybackPlan>;
  topBar: ComponentProps<typeof TopBar>;
}) {
  const { plan, startPositionMs, onCantPlay } = planned;
  const playerRef = useRef<HTMLDivElement>(null);
  const { ref: videoRef, events, ...video } = useVideo();
  const src = usePlaybackSource(videoRef, plan, onCantPlay);
  useWatchState(videoRef, versionId, startPositionMs);
  const { hidden, wake } = useControlsHidden({ playerRef, videoRef }, video);
  const nextEpisode = useNextEpisode(autoplay, video, wake);
  const { onInput, playNext } = nextEpisode;
  const next = nextLink(inShow);
  useShortcuts(
    { videoRef, playerRef },
    {
      back,
      next,
      wake: onInput,
      captions: (element) => {
        planned.chooseSubtitle(element, captionsToggled(plan, planned.shownSubtitle));
      },
      frame: frameSeconds(tracks),
    },
  );
  useMediaSession(videoRef, { title, artist, posterUrl }, next);

  return (
    <div
      ref={playerRef}
      data-controls-hidden={hidden}
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
        className="size-full object-contain picture-in-picture:opacity-0"
        {...events}
      >
        <SubtitleTracks subtitles={plan.subtitles} />
      </video>
      <FirstFrameCover backdropUrl={backdropUrl} framed={video.state.framed} />
      <PictureInPictureCover backdropUrl={backdropUrl} />
      <BufferingRing buffering={video.state.buffering} />
      <PlayerControls
        topBar={topBar}
        video={video}
        refs={{ playerRef, videoRef }}
        tracks={tracks}
        planned={planned}
        autoplay={autoplay.next !== undefined}
        inShow={inShow}
      />
      <NextEpisodeOverlay nextEpisode={nextEpisode} back={back} />
    </div>
  );
}
