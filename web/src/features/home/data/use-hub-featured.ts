import { useQueries } from "@tanstack/react-query";

import { featuredQuery } from "@/features/home/data/use-featured";
import { useReachedServers } from "@/features/home/data/use-reached-servers";
import { featuredPanels } from "@/features/home/lib/featured-panels";
import { mergeFeatured, panelOn } from "@/features/home/lib/hub-titles";

/** Featured at the Hub, each Server's in turn, from the Servers that have answered so far. */
export function useHubFeatured() {
  const reached = useReachedServers();
  // Not suspended, so a slow or failing Server holds up neither Featured nor the rest.
  return useQueries({
    queries: reached.map(({ transport }) => featuredQuery(transport)),
    combine: (answers) =>
      mergeFeatured(
        reached.map((server, at) =>
          featuredPanels(answers[at]?.data?.titles ?? []).map((panel) => panelOn(server, panel)),
        ),
      ),
  });
}
