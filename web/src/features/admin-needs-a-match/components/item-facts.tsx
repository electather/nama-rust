import { whyUnmatched } from "@/features/admin-needs-a-match/lib/why-unmatched";
import { TitleKind, type UnmatchedItem } from "@/gen/nama/v1/match_pb";

/** What an Unmatched Item's file name was `parsed` as, why it's Unmatched, and the file. */
export function ItemFacts({ item, parsed }: { item: UnmatchedItem; parsed: string }) {
  return (
    <div className="flex min-w-0 flex-1 basis-64 flex-col">
      <span className="font-medium">
        {parsed}
        <span className="ms-2 text-sm font-normal text-muted-foreground">
          {item.kind === TitleKind.SHOW ? "Episode" : "Movie"}
        </span>
      </span>
      <span className="text-sm empty:hidden">{whyUnmatched(item)}</span>
      <span className="text-sm wrap-anywhere text-muted-foreground">{item.path}</span>
    </div>
  );
}
