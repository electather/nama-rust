import { StopPlay } from "@/features/admin-now-playing/components/stop-play";
import { useViewer } from "@/features/admin-now-playing/hooks/use-viewer";
import { howItPlays, progress } from "@/features/admin-now-playing/lib/play-words";
import { reasonText } from "@/features/player/lib/playback-wording";
import type { Playback } from "@/gen/nama/v1/event_pb";
import type { Play } from "@/gen/nama/v1/server_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";

/** One play on the Now Playing list: what, who and where, how it plays and why, and how far in. */
export function PlayRow({ play, playback }: { play: Play; playback: Playback }) {
  const viewer = useViewer(playback);
  const title = play.title === "" ? "A deleted Version" : play.title;
  return (
    <AdminRow end={<StopPlay planId={playback.planId} title={title} viewer={viewer} />}>
      <p className="truncate font-medium">{title}</p>
      <span className="text-sm">{viewer}</span>
      <span className="text-sm">{howItPlays(play)}</span>
      <span className="text-sm text-muted-foreground tabular-nums">{progress(play)}</span>
      {playback.reasons.length > 0 && (
        <ul className="mt-1 text-sm text-pretty text-muted-foreground">
          {playback.reasons.map(reasonText).map((text) => (
            <li key={text}>{text}</li>
          ))}
        </ul>
      )}
    </AdminRow>
  );
}
