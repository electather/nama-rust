import { cn } from "cn";
import { TriangleAlert } from "lucide-react";
import { Button, ghostHang } from "nama-ui/components/button";

import { EmptyFolderNote } from "@/features/admin-libraries/components/empty-folder-note";
import { useScanLibrary } from "@/features/admin-libraries/data/use-scan-library";

/** The warning that `folder` looks empty, with the button that removes what's missing from it. */
export function EmptyFolder({ libraryId, folder }: { libraryId: bigint; folder: string }) {
  const scan = useScanLibrary();
  return (
    <li className="flex flex-col gap-3 rounded-xl border bg-card p-4.5 text-sm sm:flex-row sm:items-center sm:justify-between">
      <p className="flex gap-2.5 text-pretty">
        <TriangleAlert aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <EmptyFolderNote folder={folder} />
      </p>
      <div className={cn(ghostHang, "sm:ms-0")}>
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
          <p role="alert" className="ms-2.5 text-destructive">
            Couldn’t remove them. Try again in a moment.
          </p>
        )}
      </div>
    </li>
  );
}
