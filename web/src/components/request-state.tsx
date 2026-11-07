import {
  Check,
  CircleCheck,
  CircleDashed,
  CircleHelp,
  Clock,
  Download,
  type LucideIcon,
  TriangleAlert,
  X,
} from "lucide-react";

import { RequestState } from "@/gen/nama/v1/request_pb";

interface Shown {
  label: string;
  Icon: LucideIcon;
}

/** Each state in the Glossary's words, with its icon, so it never rests on colour alone. */
const states: Partial<Record<RequestState, Shown>> = {
  [RequestState.PENDING]: { label: "Pending", Icon: Clock },
  [RequestState.APPROVED]: { label: "Approved", Icon: Check },
  [RequestState.DECLINED]: { label: "Declined", Icon: X },
  [RequestState.PROCESSING]: { label: "Processing", Icon: Download },
  [RequestState.PARTIALLY_AVAILABLE]: { label: "Partially Available", Icon: CircleDashed },
  [RequestState.AVAILABLE]: { label: "Available", Icon: CircleCheck },
  [RequestState.FAILED]: { label: "Failed", Icon: TriangleAlert },
};

/** A state from a newer Server than this app knows. */
const unknown: Shown = { label: "Unknown", Icon: CircleHelp };

/** A Request's state as a labelled pill. */
export function RequestStateLabel({ state }: { state: RequestState }) {
  const { label, Icon } = states[state] ?? unknown;
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-sm text-muted-foreground">
      <Icon aria-hidden className="size-3.5" />
      {label}
    </span>
  );
}
