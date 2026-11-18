import { createFileRoute, redirect } from "@tanstack/react-router";

import { wordsParam } from "@/shared/lib/words-param";

/** The search page the action bar replaced: `?q=`, or the older `?query=`, opens it on Home. */
export const Route = createFileRoute("/_member/search")({
  validateSearch: (search): { q: string } => ({
    q: (wordsParam(search["q"]) ?? wordsParam(search["query"]) ?? "").trim(),
  }),
  beforeLoad: ({ search: { q } }) => {
    throw redirect({ to: "/", search: { search: q }, replace: true });
  },
});
