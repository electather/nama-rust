import { ProgressRing } from "nama-ui/components/progress";

import { cornerClasses } from "@/features/title/lib/corner-classes";
import type { Download } from "@/gen/nama/v1/request_pb";
import { overallPercent } from "@/shared/lib/download-progress";

/** A missing Episode's ring, filled as far as its Season's `downloads` have got. */
export function RingCorner({ downloads }: { downloads: Download[] }) {
  return (
    <span className={cornerClasses}>
      {/* Twice an icon's size, as large as the + it takes the place of. */}
      <span className="grid scale-200">
        <ProgressRing value={Math.floor(overallPercent(downloads))} aria-label="Downloading" />
      </span>
    </span>
  );
}
