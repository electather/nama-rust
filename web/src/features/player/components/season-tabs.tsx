import { useEffect, useRef } from "react";

import { seasonName } from "@/shared/lib/title-facts";
import { TabsList, TabsTab } from "@/shared/ui/tabs";

/**
 * A tab for each of the Show's Seasons, at the head of the Episodes panel, keeping the one marked,
 * `shown`, in view as the list scrolls.
 */
export function SeasonTabs({ seasons, shown }: { seasons: number[]; shown: number | undefined }) {
  const list = useRef<HTMLDivElement>(null);
  const at = seasons.indexOf(shown ?? -1);
  useEffect(() => {
    list.current?.children[at]?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [at]);
  return (
    <div className="flex-none px-5 pt-5 pb-2.5">
      <TabsList ref={list} aria-label="Season">
        {seasons.map((number) => (
          <TabsTab key={number} value={number}>
            {seasonName(number)}
          </TabsTab>
        ))}
      </TabsList>
    </div>
  );
}
