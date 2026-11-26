import { HubTitlesRow } from "@/features/home/components/hub-titles-row";
import { useHubNewest } from "@/features/home/data/use-hub-newest";
import type { RowOn } from "@/features/home/lib/hub-rows";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";

/** The newest titles of each of the Servers' Libraries of one kind, under `heading`. */
export function HubNewestRow({
  heading,
  sources,
}: {
  heading: string;
  sources: RowOn<RecentlyAddedHomeRow>[];
}) {
  return <HubTitlesRow heading={heading} titles={useHubNewest(sources)} />;
}
