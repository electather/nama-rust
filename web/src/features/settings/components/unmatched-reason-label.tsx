import {
  CircleDashed,
  Hand,
  Hash,
  ListX,
  type LucideIcon,
  SearchX,
  Split,
  Unplug,
} from "lucide-react";

import { type Reasoned, reasonCaption } from "@/features/admin-needs-a-match/lib/unmatched-reason";
import { UnmatchedReason } from "@/gen/nama/v1/match_pb";

// A reason this app doesn't know shows the icon for none, as its caption falls back.
const icons: Partial<Record<UnmatchedReason, LucideIcon>> = {
  [UnmatchedReason.AMBIGUOUS]: Split,
  [UnmatchedReason.NO_TITLE_FITS]: SearchX,
  [UnmatchedReason.UNKNOWN_ID_HINT]: Hash,
  [UnmatchedReason.NOT_IN_SHOW]: ListX,
  [UnmatchedReason.CLEARED_BY_HAND]: Hand,
  [UnmatchedReason.NO_SEARCHING_SOURCE]: Unplug,
};

/** Why `item` needs a Match, as an icon for its reason beside a few words. */
export function UnmatchedReasonLabel({ item }: { item: Reasoned }) {
  const Icon = icons[item.reason] ?? CircleDashed;
  return (
    <span className="inline-flex items-center gap-1.5">
      <Icon aria-hidden strokeWidth={1.5} className="size-3.5 shrink-0" />
      {reasonCaption(item)}
    </span>
  );
}
