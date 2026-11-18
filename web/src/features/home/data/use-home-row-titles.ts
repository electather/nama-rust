import { useQuery } from "@connectrpc/connect-query";

import { BrowseService, type HomeRow } from "@/gen/nama/v1/browse_pb";

/** A row's one page on home: whole rows of posters at two and five columns. */
const rowPage = { size: 10 };

/** The titles of one Plugin's `row`, once they have loaded. */
export function useHomeRowTitles(row: HomeRow) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- each row loads on its own, and Home never waits on one
  return useQuery(BrowseService.method.listHomeRowTitles, {
    pluginId: row.pluginId,
    key: row.key,
    page: rowPage,
  }).data?.titles;
}
