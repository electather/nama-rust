import type { MessageInitShape } from "@bufbuild/protobuf";

import { FoundSubtitleItem } from "@/features/player/components/found-subtitle-item";
import type { FoundSubtitle, FoundSubtitleSchema } from "@/gen/nama/v1/playback_pb";

/**
 * What a subtitle search found, if it's done, the one `picked` downloading while `busy`, as `pick`
 * downloads it.
 */
export function FoundSubtitles({
  found,
  busy,
  picked,
  pick,
}: {
  found: FoundSubtitle[] | undefined;
  busy: boolean;
  picked: MessageInitShape<typeof FoundSubtitleSchema> | undefined;
  pick: (subtitle: FoundSubtitle) => void;
}) {
  if (!found) {
    return null;
  }
  return (
    <ul aria-label="Found subtitles" className="flex flex-col gap-0.5">
      {found.map((each) => (
        <li key={`${each.pluginId}/${each.id}`}>
          <FoundSubtitleItem
            found={each}
            note={busy && each === picked ? "Downloading…" : ""}
            busy={busy}
            onPick={() => {
              pick(each);
            }}
          />
        </li>
      ))}
    </ul>
  );
}
