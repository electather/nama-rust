import type { ScanState } from "@/lib/events";

/** How a scan is going, as a sentence. */
export function scanSentence(scan: ScanState) {
  if (scan.$typeName === "nama.v1.ScanStarted") {
    return "Scanning…";
  }
  if (scan.$typeName === "nama.v1.ScanProgress") {
    return `Scanning: ${scan.filesDone} of ${scan.filesTotal} files`;
  }
  return `Scanned: ${scan.added} new, ${scan.updated} changed and ${scan.removed} gone`;
}
