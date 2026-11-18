import { createFileRoute } from "@tanstack/react-router";

import { SearchPage } from "@/features/action-bar/components/search-page";
import { searchQuery } from "@/features/action-bar/data/use-search-results";

/** What the nav's search box found in the signed-in User's Libraries for `?query=`. */
export const Route = createFileRoute("/_member/search")({
  validateSearch: (search): { query?: string | undefined } => {
    // The router parses `?query=1917` as JSON, so a number is words too.
    const raw = search["query"];
    const query = typeof raw === "string" || typeof raw === "number" ? String(raw).trim() : "";
    // Set even when blank: the router keeps the raw value under any key this leaves out.
    return { query: query === "" ? undefined : query };
  },
  loaderDeps: ({ search: { query } }) => ({ query }),
  // The router keeps the last results up while the next keystroke's load, so the page doesn't
  // flash, and its "Try again" loads them again.
  loader: async ({ context: { queryClient, transport }, deps: { query } }) => {
    if (query === undefined) {
      return;
    }
    await queryClient.infiniteQuery(searchQuery(query, transport));
  },
  component: Search,
});

function Search() {
  const { query } = Route.useSearch();
  return <SearchPage query={query} />;
}
