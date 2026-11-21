import { StopPlay } from "@/features/admin-now-playing/components/stop-play";
import { useViewerWords } from "@/features/admin-now-playing/hooks/use-viewer-words";
import {
  howItPlays,
  type KnownPlay,
  positionWords,
} from "@/features/admin-now-playing/lib/play-words";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { reasonText } from "@/shared/lib/playback-wording";

/** One play on the Now Playing list: what, who and where, how it plays and why, and how far in. */
export function PlayRow({ play }: { play: KnownPlay }) {
  const { playback } = play;
  const viewer = useViewerWords(playback);
  const title = play.title === "" ? "A deleted Version" : play.title;
  const position = positionWords(play);
  return (
    <AdminRow end={<StopPlay planId={playback.planId} title={title} viewer={viewer} />}>
      <p className="truncate font-medium">{title}</p>
      <span className="text-sm">{viewer}</span>
      <span className="text-sm">{howItPlays(play)}</span>
      {position !== "" && (
        <span className="text-sm text-muted-foreground tabular-nums">{position}</span>
      )}
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
