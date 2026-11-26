import { HubContinueWatching } from "@/features/home/components/hub-continue-watching";
import { HubNewestRow } from "@/features/home/components/hub-newest-row";
import { HubPluginRow } from "@/features/home/components/hub-plugin-row";
import { useHubRows } from "@/features/home/data/use-hub-rows";

/** Each kind of Home row the Account's Servers have, as each Server answers. */
export function HubRowList() {
  return useHubRows().map((row) => {
    if (row.case === "plugin") {
      return <HubPluginRow key={row.key} heading={row.heading} sources={row.sources} />;
    }
    if (row.case === "recentlyAdded") {
      return <HubNewestRow key={row.key} heading={row.heading} sources={row.sources} />;
    }
    return <HubContinueWatching key={row.key} />;
  });
}
