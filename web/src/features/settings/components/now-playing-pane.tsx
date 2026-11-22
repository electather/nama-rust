import { usePlays } from "@/features/admin-now-playing/data/use-plays";
import { hasPlayback } from "@/features/admin-now-playing/lib/play-words";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { PlayItem } from "@/features/settings/components/play-item";

/** What every User is playing now, for an Admin, kept current by the Playback events. */
export function NowPlayingPane() {
  const plays = usePlays().filter((play) => hasPlayback(play));
  return (
    <PaneFrame section="now-playing">
      {plays.length === 0 ? (
        <p className="py-3.5 text-sm text-muted-foreground">Nothing is playing.</p>
      ) : (
        <ul aria-label="Now Playing" className="-my-2.5 flex flex-col">
          {plays.map((play) => (
            <PlayItem key={play.playback.planId} play={play} />
          ))}
        </ul>
      )}
    </PaneFrame>
  );
}
