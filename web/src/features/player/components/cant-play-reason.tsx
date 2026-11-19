import { HardDrive, Tv } from "lucide-react";

import { reasonText } from "@/features/player/lib/playback-wording";
import { type PlaybackReason, PlaybackReasonKindSchema } from "@/gen/nama/v1/playback_pb";

/** A reason a Version can't play, after an icon for whose it is: the Device's, or the Server's. */
export function CantPlayReason({ reason }: { reason: PlaybackReason }) {
  // A kind newer than this app has no name, and is taken as the Server's.
  const Icon =
    PlaybackReasonKindSchema.value[reason.kind]?.name.includes("DEVICE") === true ? Tv : HardDrive;
  return (
    <li className="flex items-start gap-2.5">
      <Icon aria-hidden className="size-5 shrink-0 text-muted-foreground" />
      {reasonText(reason)}
    </li>
  );
}
