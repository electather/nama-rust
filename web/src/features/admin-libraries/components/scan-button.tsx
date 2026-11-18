import { useScanLibrary } from "@/features/admin-libraries/data/use-scan-library";
import type { Library } from "@/gen/nama/v1/library_pb";
import { Button } from "@/shared/ui/button";

/** Starts scanning `library`'s folders. Its progress arrives as Events. */
export function ScanButton({ library }: { library: Library }) {
  const scan = useScanLibrary();
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
