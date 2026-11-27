import { RowArt } from "@/features/action-bar/components/row-art";
import { RowIcon } from "@/features/action-bar/components/row-icon";
import type { Row } from "@/features/action-bar/lib/rows";
import { OnServer } from "@/shared/components/on-server";
import { Still } from "@/shared/components/still";
import type { TitleServer } from "@/shared/lib/title-server";

/**
 * What a row leads with: an Episode's Still or a title's poster, drawn from its Server `on`, or its
 * icon, as for a Recent whose Server isn't reached. Nothing for a row without an icon.
 */
export function RowLead({ row, on }: { row: Row; on: TitleServer | undefined }) {
  if (on !== undefined && row.still !== undefined) {
    // As tall as a poster, at 16:9.
    return (
      <OnServer on={on}>
        <span className="w-25 shrink-0">
          <Still still={row.still} sizes="100px" />
        </span>
      </OnServer>
    );
  }
  if (on !== undefined && row.poster) {
    return (
      <OnServer on={on}>
        <RowArt poster={row.poster} />
      </OnServer>
    );
  }
  if (row.icon === undefined) {
    return null;
  }
  return <RowIcon icon={row.icon} bare={row.poster !== undefined} />;
}
