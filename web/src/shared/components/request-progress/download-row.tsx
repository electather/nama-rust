import { CircleArrowDown, Clock, FolderInput, TriangleAlert } from "lucide-react";

import { type Download, DownloadStatus } from "@/gen/nama/v1/request_pb";
import type { Shown } from "@/shared/components/state-label";
import { timeLeftText } from "@/shared/lib/download-progress";
import { metadataLine, sizeOf } from "@/shared/lib/title-facts";

/** Each status in words, with its icon, so it never rests on colour alone. */
const statuses: Partial<Record<DownloadStatus, Shown>> = {
  [DownloadStatus.QUEUED]: { label: "Queued", Icon: Clock },
  [DownloadStatus.DOWNLOADING]: { label: "Downloading", Icon: CircleArrowDown },
  [DownloadStatus.IMPORTING]: { label: "Importing", Icon: FolderInput },
  [DownloadStatus.NEEDS_ATTENTION]: { label: "Needs attention", Icon: TriangleAlert },
};

/** One download of a Request: its title, status, how far it has got, and why it needs attention. */
export function DownloadRow({ download }: { download: Download }) {
  const { label, Icon } = statuses[download.status] ?? { label: "Unknown", Icon: Clock };
  return (
    <li className="text-sm">
      <p className="truncate">{download.title}</p>
      <p className="flex items-center gap-1.5 text-muted-foreground">
        <Icon aria-hidden className="size-3.5 shrink-0" />
        {metadataLine(label, ...facts(download))}
      </p>
      {download.status === DownloadStatus.NEEDS_ATTENTION && (
        <p className="text-pretty text-muted-foreground">
          It stopped, and whoever runs the Server needs to look at it. {download.attentionReason}
        </p>
      )}
    </li>
  );
}

/** How far a download has got, how big it is and how long it has left, skipping unknowns. */
function facts({ percent, sizeBytes, etaSeconds }: Download) {
  return [
    `${Math.floor(percent)}%`,
    sizeBytes === undefined ? undefined : sizeOf(sizeBytes),
    etaSeconds === undefined ? undefined : timeLeftText(Number(etaSeconds)),
  ];
}
