import { Link } from "@tanstack/react-router";
import { Film, Tv } from "lucide-react";

import { ItemRow } from "@/features/settings/components/item-row";
import { UnmatchedReasonLabel } from "@/features/settings/components/unmatched-reason-label";
import { TitleKind, type UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { detailSearch } from "@/shared/lib/settings-address";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * One file on a Library's list: a Movie's film or a Show's screen, what its name was parsed as
 * over why it needs a Match, as an icon and a few words. All of the row opens the file's own screen, which still has the
 * Library `picked` filtered to when Back returns.
 */
export function UnmatchedLink({
  item,
  picked,
}: {
  item: UnmatchedItem;
  picked: string | undefined;
}) {
  const name = (
    <Link
      to="."
      search={(rest) => ({
        ...rest,
        ...detailSearch({ kind: "unmatched-item", id: String(item.itemId), inLibrary: picked }),
      })}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {parsedAs(item)}
    </Link>
  );
  const icon = <IconTile Icon={item.kind === TitleKind.SHOW ? Tv : Film} />;
  return (
    <ItemRow beside linked start={icon} name={name} meta={<UnmatchedReasonLabel item={item} />} />
  );
}
