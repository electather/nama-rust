import { useEffect, useState } from "react";

import { QuickScroll } from "@/features/library/components/quick-scroll";
import { RailLayout } from "@/features/library/components/rail-layout";
import { TitleGrid } from "@/features/library/components/title-grid";
import { type IndexKey, railOf } from "@/features/library/lib/rail";
import type { GridProps } from "@/features/library/lib/titles-props";

/**
 * How many titles `usePaged` holds, 10 pages of 100; a key past them can't be scrolled to.
 */
// ponytail: keys past the first 1,000 titles are left off the rail, as `firstPageOf` notes.
const HELD = 1000;

/**
 * A Library's grid beside its quick-scroll rail, for `index`'s letters or years (`byYear`). A key
 * loads pages until its first title, then scrolls to it.
 */
export function RailedGrid({
  index,
  byYear,
  ...grid
}: GridProps & { index: IndexKey[]; byYear: boolean }) {
  const [target, setTarget] = useState<number>();
  const { paged } = grid;
  const short = target !== undefined && target >= grid.posters.length && paged.hasNextPage;
  useEffect(() => {
    if (short) {
      // Asks once however often it runs while a page is on its way.
      void paged.fetchNextPage({ cancelRefetch: false });
    }
  }, [short, paged]);
  const stops = railOf(index.filter(({ position }) => position < HELD));
  const rail = stops.length > 0 && (
    <QuickScroll stops={stops} titles={{ many: grid.titles.many, byYear }} jump={setTarget} />
  );
  return (
    <RailLayout rail={rail}>
      <TitleGrid
        {...grid}
        jump={{
          target,
          reached: () => {
            setTarget(undefined);
          },
        }}
      />
    </RailLayout>
  );
}
