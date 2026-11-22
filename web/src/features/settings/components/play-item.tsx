import { MonitorPlay } from "lucide-react";

import { StopPlay } from "@/features/admin-now-playing/components/stop-play";
import { useViewerWords } from "@/features/admin-now-playing/hooks/use-viewer-words";
import {
  howItPlays,
  type KnownPlay,
  positionWords,
} from "@/features/admin-now-playing/lib/play-words";
import { ItemRow } from "@/features/settings/components/item-row";
import { IconTile } from "@/shared/components/icon-tile";
import { reasonText } from "@/shared/lib/playback-wording";

/** One play: its title over who plays it on which Device, how and how far in, and why; then Stop. */
export function PlayItem({ play }: { play: KnownPlay }) {
  const { playback } = play;
  const viewer = useViewerWords(playback);
  const title = play.title === "" ? "A deleted Version" : play.title;
  const about = [viewer, howItPlays(play), positionWords(play)].filter((words) => words !== "");
  const meta = (
    <>
      {about.join(" · ")}
      {playback.reasons.map(reasonText).map((text) => (
        <span key={text} className="mt-0.5 block">
          {text}
        </span>
      ))}
    </>
  );
  return (
    <ItemRow beside start={<IconTile Icon={MonitorPlay} />} name={title} meta={meta}>
      <StopPlay planId={playback.planId} title={title} viewer={viewer} />
    </ItemRow>
  );
}
