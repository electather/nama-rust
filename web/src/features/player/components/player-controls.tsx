import { type ComponentProps, type RefObject, useRef } from "react";

import { BottomBar } from "@/features/player/components/bottom-bar";
import { EpisodesPanel } from "@/features/player/components/episodes-panel";
import { NextEpisodeButton } from "@/features/player/components/next-episode-button";
import { SettingsPanel } from "@/features/player/components/settings-panel";
import { TopBar } from "@/features/player/components/top-bar";
import { TracksPanel } from "@/features/player/components/tracks-panel";
import type { usePlaybackPlan } from "@/features/player/hooks/use-playback-plan";
import type { Video } from "@/features/player/hooks/use-video";
import type { InShow } from "@/features/player/lib/in-show";
import type { Track } from "@/gen/nama/v1/browse_pb";

function playedPart({ state }: Video) {
  return state.duration > 0 ? state.time / state.duration : 0;
}

/** The title bar over the video and the bottom bar under it, which fade out together. */
export function PlayerControls({
  topBar,
  video,
  refs: { playerRef, videoRef },
  tracks,
  planned,
  autoplay,
  inShow,
}: {
  topBar: ComponentProps<typeof TopBar>;
  video: Video;
  refs: {
    playerRef: RefObject<HTMLDivElement | null>;
    videoRef: RefObject<HTMLVideoElement | null>;
  };
  tracks: Track[];
  planned: ReturnType<typeof usePlaybackPlan>;
  /** Whether Playback settings offers autoplay, as it does for an Episode with one after it. */
  autoplay: boolean;
  inShow: InShow | undefined;
}) {
  const rowsRef = useRef<HTMLDivElement>(null);
  const place = { container: playerRef, anchor: rowsRef };
  return (
    // Over drawn subtitles' canvas (libbitsub's is z-10). Any key shows them, so focus rings do.
    <div className="pointer-events-none absolute inset-0 z-20 flex flex-col justify-between transition-opacity duration-500 ease-out-expo group-data-controls-hidden:opacity-0 motion-reduce:transition-none">
      <TopBar
        {...topBar}
        actions={
          inShow && (
            <EpisodesPanel inShow={inShow} container={playerRef} played={playedPart(video)} />
          )
        }
      />
      <BottomBar
        video={video}
        refs={{ playerRef, videoRef, rowsRef }}
        next={inShow?.after && <NextEpisodeButton episode={inShow.after} link={inShow.link} />}
      >
        <TracksPanel tracks={tracks} planned={planned} videoRef={videoRef} place={place} />
        <SettingsPanel
          video={video}
          planned={planned}
          tracks={tracks}
          autoplay={autoplay}
          place={place}
        />
      </BottomBar>
    </div>
  );
}
