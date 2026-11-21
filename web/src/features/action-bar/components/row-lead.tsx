import { RowArt } from "@/features/action-bar/components/row-art";
import { RowIcon } from "@/features/action-bar/components/row-icon";
import type { Row } from "@/features/action-bar/lib/rows";
import { Still } from "@/shared/components/still";

/**
 * What a row leads with: an Episode's Still, a title's poster, or its icon. Nothing for a row
 * without an icon.
 */
export function RowLead({ row }: { row: Row }) {
  if (row.still !== undefined) {
    // As tall as a poster, at 16:9.
    return (
      <span className="w-25 shrink-0">
        <Still url={row.still.url} thumbhash={row.still.thumbhash} sizes="100px" />
      </span>
    );
  }
  if (typeof row.poster === "string") {
    return <RowArt poster={row.poster} />;
  }
  if (row.icon === undefined) {
    return null;
  }
  return <RowIcon icon={row.icon} bare={row.poster === null} />;
}
