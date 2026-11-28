import { MonitorPlay } from "lucide-react";

import { usePlays } from "@/features/admin-now-playing/data/use-plays";
import { hasPlayback } from "@/features/admin-now-playing/lib/play-words";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PlayItem } from "@/features/settings/components/play-item";
import { EmptyNote } from "@/shared/components/empty-note";

/** What every User is playing now, for an Admin, kept current by the Playback events. */
export function NowPlayingPane() {
  const plays = usePlays().filter((play) => hasPlayback(play));
  return (
    <PaneFrame>
      {plays.length === 0 ? (
        <EmptyNote
          Icon={MonitorPlay}
          title="Nothing is playing"
          todo="What anyone plays on this Server shows here as it plays."
        />
      ) : (
        <ul aria-label="Now Playing" className="-my-1.5 flex flex-col">
          {plays.map((play) => (
            <PlayItem key={play.playback.planId} play={play} />
          ))}
        </ul>
      )}
    </PaneFrame>
  );
}
