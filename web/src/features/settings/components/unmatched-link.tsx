import { ChevronRight } from "lucide-react";

import { FilePoster } from "@/features/settings/components/file-poster";
import { UnmatchedName } from "@/features/settings/components/unmatched-name";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";

/**
 * One file on a Library's list: its stand-in poster, what its name was parsed as, why it needs a
 * Match in a few words and a chevron. All of the row opens the file's own screen, which still
 * has the Library `picked` filtered to when Back returns. The hairline over it starts at its name.
 */
export function UnmatchedLink({
  item,
  picked,
}: {
  item: UnmatchedItem;
  picked: string | undefined;
}) {
  return (
    <li className="group relative flex items-center gap-3.5 ps-3.5 transition-colors hover:bg-muted/50 motion-reduce:transition-none">
      <FilePoster kind={item.kind} size="row" />
      <div className="flex min-w-0 flex-1 items-center gap-3 py-2.5 pe-3.5 group-not-first:border-t">
        <UnmatchedName item={item} picked={picked} />
        <ChevronRight
          aria-hidden
          strokeWidth={1.5}
          className="size-4 shrink-0 text-muted-foreground rtl:rotate-180"
        />
      </div>
    </li>
  );
}
