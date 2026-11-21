import type { LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Autoplay } from "@/features/player/hooks/use-autoplay";
import type { InShow } from "@/features/player/lib/in-show";
import type { NowPlaying } from "@/features/player/lib/lock-screen";
import type { NextEpisode } from "@/features/player/lib/next-episode";
import type { Track, Version } from "@/gen/nama/v1/browse_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";

/** What the player plays, and where "back" goes. */
export interface PlayerProps extends NowPlaying {
  /** The Movie's or Episode's title, which also names the video. */
  title: string;
  /** The title bar's heading, when it isn't `title`: an Episode's is its Show's. */
  heading?: string | undefined;
  /** The facts under the heading, such as "2024 · PG-13" or "S1, E3 · Half Loop · TV-MA". */
  line?: string | undefined;
  versionId: bigint;
  /** The title's Versions, which it offers another of when this one can't play here. */
  versions?: Version[] | undefined;
  /** The Version's Tracks, whose audio ones the User chooses from. */
  tracks: Track[];
  /** Your Watch State of the Movie or Episode: where playing resumes. */
  watchState: WatchState | undefined;
  /** The picture behind the player until its first frame: a Movie's Backdrop, an Episode's Still. */
  backdropUrl?: string | undefined;
  /** The title page to go back to. */
  back: LinkOptions;
  /** The Episode to count down to and play after this one, if there is one. */
  next?: NextEpisode | undefined;
  /** Where an Episode plays in its Show; a Movie has none. */
  inShow?: InShow | undefined;
  /** A note under the title bar while it plays, such as that progress isn't saved. */
  note?: ReactNode;
}

/** What the player plays once it has an `autoplay` to keep. */
export type PlannedProps = PlayerProps & { autoplay: Autoplay };
