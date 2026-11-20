import { CircleArrowDown } from "lucide-react";

import type { Download } from "@/gen/nama/v1/request_pb";
import { overallPercent, slowestTimeLeft } from "@/shared/lib/download-progress";
import { metadataLine } from "@/shared/lib/title-facts";
import { ProgressPill } from "@/shared/ui/progress";

/** "Downloading", filled to how far `downloads` have got, with how long the slowest has left. */
export function DownloadingPill({ downloads }: { downloads: Download[] }) {
  return (
    <ProgressPill
      value={Math.floor(overallPercent(downloads))}
      label={metadataLine("Downloading", slowestTimeLeft(downloads))}
    >
      <CircleArrowDown aria-hidden className="size-3.5 shrink-0" />
    </ProgressPill>
  );
}
