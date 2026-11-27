import { useState } from "react";

import { FeaturedFrame } from "@/features/home/components/featured-frame";
import { FeaturedPanel } from "@/features/home/components/featured-panel";
import { OnServer } from "@/features/home/components/on-server";
import type { PanelOn } from "@/features/home/lib/featured-panels";

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
      {panels.map(({ transport, panel }, index) => (
        <OnServer key={panel.key} transport={transport}>
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
