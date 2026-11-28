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
import { Tooltip, TooltipContent, TooltipTrigger } from "nama-ui/components/tooltip";

import { type Reasoned, reasonCaption } from "@/features/admin-needs-a-match/lib/unmatched-reason";
import { UnmatchedReason } from "@/gen/nama/v1/match_pb";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

// A reason this app doesn't know shows the icon for none, as its caption falls back.
const icons: Partial<Record<UnmatchedReason, LucideIcon>> = {
  [UnmatchedReason.AMBIGUOUS]: Split,
  [UnmatchedReason.NO_TITLE_FITS]: SearchX,
  [UnmatchedReason.UNKNOWN_ID_HINT]: Hash,
  [UnmatchedReason.NOT_IN_SHOW]: ListX,
  [UnmatchedReason.CLEARED_BY_HAND]: Hand,
  [UnmatchedReason.NO_SEARCHING_SOURCE]: Unplug,
};

/** Why `item` needs a Match as its reason's icon alone, named in a few words on hover and focus. */
export function UnmatchedReasonLabel({ item }: { item: Reasoned }) {
  const Icon = icons[item.reason] ?? CircleDashed;
  const label = reasonCaption(item);
  const [trigger, container] = useModalContainer();
  return (
    <Tooltip>
      {/* Not a button, as pressing it does nothing; focusable so a keyboard opens its tooltip. */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- see above */}
      <TooltipTrigger render={<span ref={trigger} tabIndex={0} />}>
        <Icon aria-hidden strokeWidth={1.5} className="block size-4 text-muted-foreground" />
        <span className="sr-only">{label}</span>
      </TooltipTrigger>
      <TooltipContent container={container}>{label}</TooltipContent>
    </Tooltip>
  );
}
