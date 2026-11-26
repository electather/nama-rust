import { createQueryOptions } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";

import { rowPage } from "@/features/home/data/use-home-row-titles";
import type { RowOn } from "@/features/home/lib/hub-rows";
import { mergeTitles } from "@/features/home/lib/hub-titles";
import { BrowseService, type PluginHomeRow } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { rowTitles } from "@/shared/lib/row-titles";

/**
 * The titles of one Plugin's row at the Hub, from each Server that gives it, as one row, as each
 * answers.
 */
export function useHubRowTitles(sources: RowOn<PluginHomeRow>[]) {
  // shortcut: only each Server's first page is merged; page through each when a shelf runs out.
  return useQueries({
    queries: sources.map(({ transport, row }) => ({
      ...createQueryOptions(
        BrowseService.method.listHomeRowTitles,
        { pluginId: row.pluginId, key: row.key, page: rowPage },
        { transport },
      ),
      staleTime: loaderStaleTime,
    })),
    combine: (answers) =>
      mergeTitles(
        sources.map(({ server, transport }, at) =>
          rowTitles(answers[at]?.data?.titles ?? []).map((title) => ({ server, transport, title })),
        ),
      ),
  });
}
