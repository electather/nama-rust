import { rowPage } from "@/features/home/lib/shelf-tiles";
import { BrowseService, type PluginHomeRow } from "@/gen/nama/v1/browse_pb";
import { usePaged } from "@/shared/hooks/use-paged";

/** The titles of one Plugin's `row`, page by page as its shelf scrolls. */
export function useHomeRowTitles(row: PluginHomeRow) {
  return usePaged(BrowseService.method.listHomeRowTitles, {
    pluginId: row.pluginId,
    key: row.key,
    page: rowPage,
  });
}
