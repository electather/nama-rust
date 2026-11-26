import { HubTitlesRow } from "@/features/home/components/hub-titles-row";
import { useHubRowTitles } from "@/features/home/data/use-hub-row-titles";
import type { RowOn } from "@/features/home/lib/hub-rows";
import type { PluginHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Plugin's row at the Hub, from each Server that gives it, under `heading`. */
export function HubPluginRow({
  heading,
  sources,
}: {
  heading: string;
  sources: RowOn<PluginHomeRow>[];
}) {
  return <HubTitlesRow heading={heading} titles={useHubRowTitles(sources)} />;
}
