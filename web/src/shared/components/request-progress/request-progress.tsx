import type { Download, Request } from "@/gen/nama/v1/request_pb";
import { DownloadRow } from "@/shared/components/request-progress/download-row";
import { Progress } from "@/shared/ui/progress";

/**
 * How far a Request's downloads have got: one bar for them all, then each download.
 * Nothing until its Fulfiller has said what it is downloading.
 */
export function RequestProgress({ request }: { request: Request | undefined }) {
  const downloads = request?.downloads ?? [];
  if (downloads.length === 0) {
    return null;
  }
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
  let total = 0;
  let done = 0;
  for (const download of downloads) {
    total += weight(download);
    done += weight(download) * download.percent;
  }
  return done / total;
}
