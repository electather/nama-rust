import { InfoRow } from "@/features/player/components/info-row";
import { TrackChangeLine } from "@/features/player/components/track-change-line";
import type { trackChange } from "@/features/player/lib/track-change";

/** A Track's row of Playback info, as it plays and as the file holds it, if it plays one. */
export function ChangeRow({
  name,
  change,
}: {
  name: string;
  change: ReturnType<typeof trackChange> | undefined;
}) {
  return (
    <InfoRow name={name}>
      {change === undefined ? undefined : <TrackChangeLine {...change} />}
    </InfoRow>
  );
}
