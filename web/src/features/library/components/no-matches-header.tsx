import { FunnelX } from "lucide-react";
import { EmptyHeader, EmptyMedia, EmptyTitle } from "nama-ui/components/empty";

/** The icon and words of `NoMatches`: that no `titles` pass the filters. */
export function NoMatchesHeader({ titles }: { titles: string }) {
  return (
    <EmptyHeader>
      <EmptyMedia>
        <FunnelX aria-hidden strokeWidth={1.5} />
      </EmptyMedia>
      <EmptyTitle>No {titles} match these filters</EmptyTitle>
    </EmptyHeader>
  );
}
