import { ScanBar } from "@/features/admin-libraries/components/scan-bar";
import { useScanState } from "@/features/admin-libraries/data/use-scan-state";
import { scanSentence } from "@/features/admin-libraries/lib/scan-sentence";

/**
 * How the Library `libraryId`'s scan is going, as its Events say. Before the first, nothing, or
 * `idle` when it's given.
 */
export function ScanStatus({ libraryId, idle }: { libraryId: bigint; idle?: string }) {
  const scan = useScanState(libraryId);
  const sentence = scan ? scanSentence(scan) : idle;
  return (
    // A live region, so a screen reader hears the scan move on.
    <output className="flex flex-col gap-1.5 text-sm text-muted-foreground empty:hidden">
      {sentence !== undefined && <p>{sentence}</p>}
      <ScanBar scan={scan} />
    </output>
  );
}
