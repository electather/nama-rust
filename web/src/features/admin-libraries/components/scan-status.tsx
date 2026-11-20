import { ScanBar } from "@/features/admin-libraries/components/scan-bar";
import { useScanState } from "@/features/admin-libraries/data/use-scan-state";
import { lastScanSentence, scanSentence } from "@/features/admin-libraries/lib/scan-sentence";
import type { LastScan } from "@/gen/nama/v1/library_pb";

/**
 * How the Library `libraryId`'s scan is going, as its Events say. Before the first, its
 * `lastScan`, or else nothing, or `idle` when it's given.
 */
export function ScanStatus({
  libraryId,
  lastScan,
  idle,
}: {
  libraryId: bigint;
  lastScan: LastScan | undefined;
  idle?: string;
}) {
  const scan = useScanState(libraryId);
  const stored = lastScan ? lastScanSentence(lastScan) : idle;
  const sentence = scan ? scanSentence(scan) : stored;
  return (
    // A live region, so a screen reader hears the scan move on.
    <output className="flex flex-col gap-1.5 text-sm text-muted-foreground empty:hidden">
      {sentence !== undefined && <p>{sentence}</p>}
      <ScanBar scan={scan} />
    </output>
  );
}
