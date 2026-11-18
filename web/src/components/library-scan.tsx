import { useMutation } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";
import { TriangleAlert } from "lucide-react";

import { type Library, LibraryService } from "@/gen/nama/v1/library_pb";
import { type ScanState, scanQuery } from "@/lib/events";
import { Button } from "@/shared/ui/button";

/**
 * How the Library `libraryId`'s scan is going, as its Events say. Before the first, nothing, or
 * `idle` when it's given.
 */
export function ScanStatus({ libraryId, idle }: { libraryId: bigint; idle?: string }) {
  const { data: scan } = useQuery(scanQuery(libraryId));
  const sentence = scan ? scanSentence(scan) : idle;
  return (
    // A live region, so a screen reader hears the scan move on.
    <output className="flex flex-col gap-1.5 text-sm text-muted-foreground empty:hidden">
      {sentence !== undefined && <p>{sentence}</p>}
      <ScanBar scan={scan} />
    </output>
  );
}

/** The scan's progress as a bar, while it's counting files. */
function ScanBar({ scan }: { scan: ScanState | undefined }) {
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

function scanSentence(scan: ScanState) {
  if (scan.$typeName === "nama.v1.ScanStarted") {
    return "Scanning…";
  }
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

/**
 * A warning for each of `library`'s folders that held files and read empty at its last scan, as an
 * unmounted drive does, with a way to remove what's missing from it once the Admin knows it's gone.
 */
export function EmptyFolders({ library }: { library: Library }) {
  if (library.emptyFolders.length === 0) {
    return null;
  }
  return (
    <ul aria-label="Folders that look empty" className="mt-6 flex max-w-xl flex-col gap-3">
      {library.emptyFolders.map((folder) => (
        <EmptyFolder key={folder} libraryId={library.id} folder={folder} />
      ))}
    </ul>
  );
}

function EmptyFolder({ libraryId, folder }: { libraryId: bigint; folder: string }) {
  const scan = useMutation(LibraryService.method.scanLibrary);
  return (
    <li className="flex flex-col gap-3 rounded-xl border bg-surface-raised p-4.5 text-sm sm:flex-row sm:items-center sm:justify-between">
      <p className="flex gap-2.5 text-pretty">
        <TriangleAlert aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <span>
          <span className="font-medium wrap-anywhere">{folder}</span> looks empty. Is the drive
          connected? Nothing was removed.
        </span>
      </p>
      <div className="-ms-4 sm:ms-0">
        <Button
          variant="ghost"
          disabled={scan.isPending}
          onClick={() => {
            scan.mutate({ id: libraryId, removeMissingIn: folder });
          }}
        >
          Remove missing items
        </Button>
        {scan.isError && (
          <p role="alert" className="ms-4 text-destructive">
            Couldn’t remove them. Try again in a moment.
          </p>
        )}
      </div>
    </li>
  );
}
