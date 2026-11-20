import {
  Check,
  CircleCheck,
  CircleDashed,
  CircleHelp,
  Clock,
  Download,
  TriangleAlert,
  X,
} from "lucide-react";

import { RequestState } from "@/gen/nama/v1/request_pb";
import type { Shown } from "@/shared/components/state-label";

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

/** A Request's state as a pill shows it. */
export function shownRequestState(state: RequestState) {
  return states[state] ?? unknown;
}

/** On a title, Pending and Approved say what's left to wait on for whoever asked. */
const onTitle: Partial<Record<RequestState, string>> = {
  [RequestState.PENDING]: "Pending approval",
  [RequestState.APPROVED]: "Requested",
};

/** The states a title shows in red: turned down, or stalled. */
const red = new Set<RequestState>([RequestState.DECLINED, RequestState.FAILED]);

/** A Request's state as a title's pill shows it. */
export function shownOnTitle(state: RequestState) {
  const shown = shownRequestState(state);
  const tone = red.has(state) ? ("destructive" as const) : ("quiet" as const);
  return { ...shown, label: onTitle[state] ?? shown.label, tone };
}

/** A Request's state in the Glossary's words. */
export function requestStateName(state: RequestState) {
  return shownRequestState(state).label;
}
