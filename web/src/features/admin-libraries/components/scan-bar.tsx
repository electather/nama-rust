import type { ScanState } from "@/shared/hooks/scan-query";

/** The scan's progress as a bar, while it's counting files. */
export function ScanBar({ scan }: { scan: ScanState | undefined }) {
  if (scan?.$typeName !== "nama.v1.ScanProgress") {
    return null;
  }
  return (
    <progress
      value={Number(scan.filesDone)}
      max={Number(scan.filesTotal)}
      aria-label="Scan progress"
      className="h-1 w-full max-w-48 accent-primary"
    />
  );
}
