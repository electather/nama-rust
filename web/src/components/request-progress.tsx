import { CircleArrowDown, Clock, FolderInput, TriangleAlert } from "lucide-react";

import type { Shown } from "@/components/state-label";
import { Progress } from "@/components/ui/progress";
import { type Download, DownloadStatus, type Request } from "@/gen/nama/v1/request_pb";
import { metadataLine, runtimeOf, sizeOf } from "@/lib/browse";

/** Each status in words, with its icon, so it never rests on colour alone. */
const statuses: Partial<Record<DownloadStatus, Shown>> = {
  [DownloadStatus.QUEUED]: { label: "Queued", Icon: Clock },
  [DownloadStatus.DOWNLOADING]: { label: "Downloading", Icon: CircleArrowDown },
  [DownloadStatus.IMPORTING]: { label: "Importing", Icon: FolderInput },
  [DownloadStatus.NEEDS_ATTENTION]: { label: "Needs attention", Icon: TriangleAlert },
};

/**
 * How far a Request's downloads have got: one bar for them all, then each download.
 * Nothing until its Fulfiller has said what it is downloading.
 */
export function RequestProgress({ request }: { request: Request | undefined }) {
  const downloads = request?.downloads ?? [];
  if (downloads.length === 0) return null;
  const overall = Math.floor(overallPercent(downloads));
  return (
    <div className="mt-2 flex w-full flex-col gap-1.5">
      <Progress value={overall} aria-label="Downloaded" />
      <p className="text-sm text-muted-foreground">{overall}% downloaded</p>
      <ul aria-label="Downloads" className="flex flex-col gap-1.5">
        {downloads.map((download, index) => (
          // oxlint-disable-next-line react/no-array-index-key -- a service may name two downloads alike, and the list only changes as a whole
          <DownloadRow key={index} download={download} />
        ))}
      </ul>
    </div>
  );
}

/** Each download's percent weighted by its size, or counted evenly unless every size is known. */
function overallPercent(downloads: Download[]) {
  const sized = downloads.every(({ sizeBytes }) => sizeBytes !== undefined && sizeBytes > 0n);
  const weight = ({ sizeBytes }: Download) => (sized ? Number(sizeBytes) : 1);
  const total = downloads.reduce((sum, download) => sum + weight(download), 0);
  const done = downloads.reduce((sum, download) => sum + weight(download) * download.percent, 0);
  return done / total;
}

function DownloadRow({ download }: { download: Download }) {
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
    etaSeconds === undefined ? undefined : `${runtimeOf(Math.ceil(Number(etaSeconds) / 60))} left`,
  ];
}
