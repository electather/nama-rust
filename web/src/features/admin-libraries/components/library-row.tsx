import { Link } from "@tanstack/react-router";

import { ScanButton } from "@/features/admin-libraries/components/scan-button";
import { ScanStatus } from "@/features/admin-libraries/components/scan-status";
import type { Library } from "@/gen/nama/v1/library_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { kindName } from "@/shared/lib/library-kind";

/** `library` on the Libraries page: a link to its page, what it holds, and its scan. */
export function LibraryRow({ library }: { library: Library }) {
  const folders = library.folders.length;
  return (
    <AdminRow
      end={
        <div className="flex items-center gap-3">
          <ScanStatus libraryId={library.id} />
          <ScanButton library={library} />
        </div>
      }
    >
      <Link
        to="/admin/libraries/$libraryId"
        params={{ libraryId: String(library.id) }}
        className="truncate rounded-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {library.name}
      </Link>
      <span className="text-sm text-muted-foreground">
        {kindName(library.kind)} · {folders} {folders === 1 ? "folder" : "folders"}
      </span>
    </AdminRow>
  );
}
