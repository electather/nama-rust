import { useEffect } from "react";

/**
 * Loads the next page once `shown`, the last index in the page, reaches `count`'s end, and again
 * each time a page arrives while it still does.
 */
export function useLoadAtEnd({
  shown,
  count,
  next,
}: {
  shown: number;
  count: number;
  next: { has: boolean; load: (options: { cancelRefetch: boolean }) => Promise<unknown> };
}) {
  const { has, load } = next;
  const atEnd = has && shown >= count - 1;
  useEffect(() => {
    if (atEnd) {
      // Scrolling on while a page loads must not ask for it twice.
      void load({ cancelRefetch: false });
    }
    // oxlint-disable-next-line react/exhaustive-effect-dependencies -- `count` re-runs it when a page arrives and the shelf is still at its end
  }, [atEnd, count, load]);
}
