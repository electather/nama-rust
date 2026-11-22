import type { LastScan } from "@/gen/nama/v1/library_pb";
import type { ScanState } from "@/shared/hooks/scan-query";

/** How a scan is going, as a sentence. */
export function scanSentence(scan: ScanState) {
  if (scan.$typeName === "nama.v1.ScanStarted") {
    return "Scanning…";
  }
  if (scan.$typeName === "nama.v1.ScanProgress") {
    return `Scanning: ${scan.filesDone} of ${scan.filesTotal} files`;
  }
  return `Scanned: ${foundCounts(scan)}`;
}

/** When a Library's last finished scan ended and what it found, as a sentence. */
export function lastScanSentence(last: LastScan) {
  return `${lastScanned(last)}: ${foundCounts(last)}`;
}

/** When a Library's last finished scan ended, as "Last scanned <when>". */
export function lastScanned({ finishedAt }: LastScan) {
  const at = new Date(Number(finishedAt) * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
  return `Last scanned ${at}`;
}

function foundCounts({
  added,
  updated,
  removed,
}: {
  added: bigint;
  updated: bigint;
  removed: bigint;
}) {
  return `${added} new, ${updated} changed and ${removed} gone`;
}
