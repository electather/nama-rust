import { Link } from "@tanstack/react-router";
import { Film, Tv } from "lucide-react";

import { ScanButton } from "@/features/admin-libraries/components/scan-button";
import { ScanStatus } from "@/features/admin-libraries/components/scan-status";
import { lastScanned } from "@/features/admin-libraries/lib/scan-sentence";
import { ItemRow } from "@/features/settings/components/item-row";
import { type Library, LibraryKind } from "@/gen/nama/v1/library_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { kindName } from "@/shared/lib/library-kind";

/**
 * One Library on the list: its name over what it holds, its folders and its last scan, and at the
 * end how a scan going on now is doing, and Scan. All of the row opens it but Scan.
 */
export function LibraryItem({ library }: { library: Library }) {
  const folders = library.folders.length;
  const about = [
    kindName(library.kind),
    `${folders} ${folders === 1 ? "folder" : "folders"}`,
    library.lastScan && lastScanned(library.lastScan),
  ].filter((words) => words !== undefined);
  const name = (
    <Link
      to="."
      search={(rest) => ({ ...rest, library: String(library.id) })}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {library.name}
    </Link>
  );
  const icon = <IconTile Icon={library.kind === LibraryKind.SHOWS ? Tv : Film} />;
  return (
    <ItemRow beside linked start={icon} name={name} meta={about.join(" · ")}>
      <ScanStatus libraryId={library.id} lastScan={undefined} />
      <ScanButton library={library} />
    </ItemRow>
  );
}
