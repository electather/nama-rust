import type { ServerImage } from "@/shared/lib/server-image";

/**
 * How every card of a Show's row is drawn: square when any of its Episodes has a synopsis, so the
 * row keeps one height, and `backdrop`, the Show's, standing in for a missing or failed Still.
 */
export interface RowLook {
  tall: boolean;
  backdrop: ServerImage | undefined;
}
