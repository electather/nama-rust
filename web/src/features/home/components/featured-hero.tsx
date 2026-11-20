import { useState } from "react";

import { FeaturedFrame } from "@/features/home/components/featured-frame";
import { FeaturedPanel } from "@/features/home/components/featured-panel";
import { useFeatured } from "@/features/home/data/use-featured";
import { featuredPanels } from "@/features/home/lib/featured-panels";

/**
 * Your Featured as a row of panels, one open at a time: hovering, focusing or pressing a closed
 * one opens it. Nothing when there's none.
 */
export function FeaturedHero() {
  const panels = featuredPanels(useFeatured());
  const [open, setOpen] = useState(0);
  if (panels.length === 0) {
    return null;
  }
  return (
    <FeaturedFrame label="Featured">
      {panels.map((panel, index) => (
        <FeaturedPanel
          key={panel.key}
          panel={panel}
          place={{ index, open, count: panels.length }}
          onOpen={() => {
            setOpen(index);
          }}
        />
      ))}
    </FeaturedFrame>
  );
}
