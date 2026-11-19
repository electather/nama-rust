import { createFileRoute, redirect } from "@tanstack/react-router";

import { wordsParam } from "@/shared/lib/words-param";

/**
 * Where the search page was. A link to it, by `?q=` or the older `?query=`, lands on Home with the
 * action bar open on those words: the action bar is the only search now.
 */
export const Route = createFileRoute("/_member/search")({
  // Set even when blank: the router keeps the raw value under any key this leaves out.
  validateSearch: (search): { q?: string | undefined } => ({
    q: wordsParam(search["q"] ?? search["query"]),
  }),
  beforeLoad: ({ search }) => {
    throw redirect({ to: "/", search: { search: search.q }, replace: true });
  },
});
