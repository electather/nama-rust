import { PlayRow } from "@/features/admin-now-playing/components/play-row";
import { usePlays } from "@/features/admin-now-playing/data/use-plays";
import { isKnown } from "@/features/admin-now-playing/lib/play-words";
import { AdminList } from "@/shared/components/admin-list/admin-list";
import { Page } from "@/shared/components/page";

/** The Now Playing page: what every User is playing now, kept current by the Playback events. */
export function NowPlaying() {
  const plays = usePlays();
  return (
    <Page heading="Now Playing">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        {plays.length === 0
          ? "Nothing is playing."
          : "What everyone is playing now, on which Device, and how."}
      </p>
      {plays.length > 0 && (
        <AdminList>
          {plays
            .filter((play) => isKnown(play))
            .map((play) => (
              <PlayRow key={play.playback.planId} play={play} />
            ))}
        </AdminList>
      )}
    </Page>
  );
}
