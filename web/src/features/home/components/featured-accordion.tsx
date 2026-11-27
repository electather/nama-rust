import { useState } from "react";

import { FeaturedFrame } from "@/features/home/components/featured-frame";
import { FeaturedPanel } from "@/features/home/components/featured-panel";
import type { PanelOn } from "@/features/home/lib/featured-panels";
import { OnServer } from "@/shared/components/on-server";

/**
 * Featured's `panels` as a row, one open at a time: hovering, focusing or pressing a closed one
 * opens it. Each runs against its own Server. Nothing when there's none.
 */
export function FeaturedAccordion({ panels }: { panels: PanelOn[] }) {
  const [open, setOpen] = useState(0);
  if (panels.length === 0) {
    return null;
  }
  return (
    <FeaturedFrame label="Featured">
      {panels.map(({ panel, ...on }, index) => (
        <OnServer key={panel.key} on={on}>
          <FeaturedPanel
            panel={panel}
            place={{ index, open, count: panels.length }}
            onOpen={() => {
              setOpen(index);
            }}
          />
        </OnServer>
      ))}
    </FeaturedFrame>
  );
}
