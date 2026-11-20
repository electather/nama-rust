import type { Hiding } from "@/features/home/lib/hiding";
import type { Marking } from "@/features/home/lib/marking";

/** What marking `marking` watched asks of the Server. */
export const watched = (marking: Marking) => ({ ...marking, played: true });

/** What removing `hiding` from Continue Watching asks of the Server. */
export const removed = (hiding: Hiding) => ({ ...hiding, hidden: true });

/**
 * `asked` as text, to find a pending request among others: two asks of the same card read the
 * same, since `watched` and `removed` build them alike. JSON has no bigint, so ids go as text.
 */
export function askedText(asked: unknown) {
  return JSON.stringify(asked, (_key, value: unknown) =>
    typeof value === "bigint" ? String(value) : value,
  );
}
