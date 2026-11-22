import { whyUnmatched } from "@/features/admin-needs-a-match/lib/why-unmatched";
import { TitleKind, type UnmatchedItem } from "@/gen/nama/v1/match_pb";

/** An Unmatched Item's file: what kind of title it was parsed as, where it is, and why it's Unmatched. */
export function ItemFacts({ item }: { item: UnmatchedItem }) {
  return (
    <>
      {item.kind === TitleKind.SHOW ? "Episode" : "Movie"} ·{" "}
      <span className="wrap-anywhere">{item.path}</span>
      <span className="mt-0.5 block empty:hidden">{whyUnmatched(item)}</span>
    </>
  );
}
