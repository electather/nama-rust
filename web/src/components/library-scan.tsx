import { useMutation } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import type { Library } from "@/gen/nama/v1/library_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { type ScanState, scanQuery } from "@/lib/events";

/** How the Library `libraryId`'s scan is going, as its Events say. Nothing before the first. */
export function ScanStatus({ libraryId }: { libraryId: bigint }) {
  const { data: scan } = useQuery(scanQuery(libraryId));
  return (
    // A live region, so a screen reader hears the scan move on.
    <output className="flex flex-col gap-1.5 text-sm text-muted-foreground empty:hidden">
      {scan && <p>{scanSentence(scan)}</p>}
      {scan?.$typeName === "nama.v1.ScanProgress" && (
        <progress
          value={Number(scan.filesDone)}
          max={Number(scan.filesTotal)}
          aria-label="Scan progress"
          className="h-1 w-full max-w-48 accent-primary"
        />
      )}
    </output>
  );
}

function scanSentence(scan: ScanState) {
  if (scan.$typeName === "nama.v1.ScanStarted") return "Scanning…";
  if (scan.$typeName === "nama.v1.ScanProgress") {
    return `Scanning: ${scan.filesDone} of ${scan.filesTotal} files`;
  }
  return `Scanned: ${scan.added} new, ${scan.updated} changed and ${scan.removed} gone`;
}

/** Starts scanning `library`'s folders. Its progress arrives as Events. */
export function ScanButton({ library }: { library: Library }) {
  const scan = useMutation(LibraryService.method.scanLibrary);
  return (
    <>
      <Button
        variant="ghost"
        aria-label={`Scan ${library.name}`}
        disabled={scan.isPending}
        onClick={() => {
          scan.mutate({ id: library.id });
        }}
      >
        Scan
      </Button>
      {scan.isError && (
        <p role="alert" className="text-sm text-destructive">
          Couldn’t start the scan. Try again in a moment.
        </p>
      )}
    </>
  );
}
