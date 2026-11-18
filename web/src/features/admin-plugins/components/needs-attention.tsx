import { TriangleAlert } from "lucide-react";

import { StateLabel } from "@/shared/components/state-label";

/** Marks an enabled Plugin missing a required setting, such as one an upgrade added. */
export function NeedsAttention() {
  return (
    <div className="mt-1.5">
      <StateLabel label="Needs attention" Icon={TriangleAlert} />
    </div>
  );
}
