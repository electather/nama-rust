import type { LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Autoplay } from "@/features/player/hooks/use-autoplay";
import type { NowPlaying } from "@/features/player/lib/lock-screen";
import type { NextEpisode } from "@/features/player/lib/next-episode";
import type { Track } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** What the player plays, and where "back" goes. */
export interface PlayerProps extends NowPlaying {
  /** The Movie's or Episode's title, which also names the video. */
  title: string;
  versionId: bigint;
  /** The Version's Tracks, whose audio ones the User chooses from. */
  tracks: Track[];
  /** Your Watch State of the Movie or Episode: where playing resumes. */
  watchState: WatchState | undefined;
  /** The title page to go back to. */
  back: LinkOptions;
  /** The Episode to count down to and play after this one, if there is one. */
  next?: NextEpisode | undefined;
  /** A note under the title bar while it plays, such as that progress isn't saved. */
  note?: ReactNode;
}

/** What the player plays once it has an `autoplay` to keep. */
export type PlannedProps = PlayerProps & { autoplay: Autoplay };
