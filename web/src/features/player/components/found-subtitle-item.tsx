import { create } from "@bufbuild/protobuf";

import { ChoiceLabel } from "@/features/player/components/choice-label";
import { trackLabel } from "@/features/player/lib/track-label";
import { TrackSchema } from "@/gen/nama/v1/browse_pb";
import type { FoundSubtitle } from "@/gen/nama/v1/playback_pb";
import { metadataLine } from "@/shared/lib/title-facts";

/**
 * A subtitle a search found, named as its Track will be, with `note` said after its details.
 * While `busy` it stays focusable but does nothing.
 */
export function FoundSubtitleItem({
  found,
  note,
  busy,
  onPick,
}: {
  found: FoundSubtitle;
  note: string;
  busy: boolean;
  onPick: () => void;
}) {
  const { language, name: title, sdh } = found;
  const { name, details } = trackLabel(create(TrackSchema, { language, title, sdh }));
  return (
    <button
      type="button"
      aria-disabled={busy}
      onClick={busy ? undefined : onPick}
      className="flex min-h-10 w-full cursor-pointer items-center rounded-md px-2.5 py-2 text-start wrap-anywhere transition-colors outline-none select-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 aria-disabled:cursor-default"
    >
      <span>
        <ChoiceLabel name={name} details={metadataLine(details, note)} />
      </span>
    </button>
  );
}
