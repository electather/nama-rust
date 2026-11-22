import { CircleArrowDown } from "lucide-react";
import { ProgressPill } from "nama-ui/components/progress";

import type { Download } from "@/gen/nama/v1/request_pb";
import { overallPercent, slowestTimeLeft } from "@/shared/lib/download-progress";
import { metadataLine } from "@/shared/lib/title-facts";

/**
 * "Downloading", filled to how far `downloads` have got, with how long the slowest has left;
 * `large` beside a title's Play.
 */
export function DownloadingPill({ downloads, large }: { downloads: Download[]; large: boolean }) {
  return (
    <ProgressPill
      value={Math.floor(overallPercent(downloads))}
      label={metadataLine("Downloading", slowestTimeLeft(downloads))}
      large={large}
    >
      <CircleArrowDown aria-hidden className={large ? "size-4.5 shrink-0" : "size-3.5 shrink-0"} />
    </ProgressPill>
  );
}
