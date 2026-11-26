import { Suspense, useEffect } from "react";

import { GettingReady } from "@/features/player/components/getting-ready";
import { PlannedPlayer } from "@/features/player/components/planned-player";
import { useAutoplay } from "@/features/player/hooks/use-autoplay";
import type { PlayerProps } from "@/features/player/lib/player-props";
import { PageTitle } from "@/shared/components/page-title";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { playerOpened } from "@/shared/lib/diagnostics";

/**
 * Plays a Version full screen with Nama's own controls, from where the Server's Playback Plan says,
 * asking for a new plan when the browser fails to play one, or says so plainly when it can't play
 * here. While the Server works a plan out, which the first time can take minutes, it says so.
 */
export function Player(played: PlayerProps) {
  const props = {
    ...played,
    posterUrl: useServerUrl(played.posterUrl),
    backdropUrl: useServerUrl(played.backdropUrl),
  };
  useEffect(playerOpened, []);
  const playing = props.heading === undefined ? props.title : `${props.heading}: ${props.title}`;
  return (
    <>
      <PageTitle name={`Playing ${playing}`} />
      {/* Above the component owning `usePlaybackPlan`'s state: a later plan then suspends it
          mounted, and React keeps the Tracks and Playback Limit it asked for. Keyed, so another
          Version starts afresh. */}
      <Suspense fallback={<GettingReady {...props} />}>
        <PlannedPlayer
          key={String(props.versionId)}
          {...props}
          autoplay={useAutoplay(props.next)}
        />
      </Suspense>
    </>
  );
}
