import { RowArt } from "@/features/action-bar/components/row-art";
import { RowIcon } from "@/features/action-bar/components/row-icon";
import type { Row } from "@/features/action-bar/lib/rows";

/** What a row leads with: a title's poster, or its icon. Nothing for a row without an icon. */
export function RowLead({ row }: { row: Row }) {
  if (typeof row.poster === "string") {
    return <RowArt poster={row.poster} />;
  }
  if (row.icon === undefined) {
    return null;
  }
  return <RowIcon icon={row.icon} bare={row.poster === null} />;
}
