import { RefreshCw } from "lucide-react";

import { useScanLibrary } from "@/features/admin-libraries/data/use-scan-library";
import type { Library } from "@/gen/nama/v1/library_pb";
import { IconAction } from "@/shared/components/icon-action";

/** Starts scanning `library`'s folders, as an icon. Its progress arrives as Events. */
export function ScanButton({ library }: { library: Library }) {
  const scan = useScanLibrary();
  return (
    <>
      <IconAction
        label="Scan"
        Icon={RefreshCw}
        aria-label={`Scan ${library.name}`}
        disabled={scan.isPending}
        onClick={() => {
          scan.mutate({ id: library.id });
        }}
      />
      {scan.isError && (
        <p role="alert" className="text-sm text-destructive">
          Couldn’t start the scan. Try again in a moment.
        </p>
      )}
    </>
  );
}
