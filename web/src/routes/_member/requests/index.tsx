import { createFileRoute, redirect } from "@tanstack/react-router";

import { RequestsPage } from "@/features/requests/components/requests-page";
import { requestsQuery } from "@/features/requests/data/use-requests";
import { defaultFilter } from "@/features/requests/lib/default-filter";
import { type StateFilter, isStateFilter } from "@/features/requests/lib/state-filter";

export const Route = createFileRoute("/_member/requests/")({
  beforeLoad: async ({ context, location, search }) => {
    // Links Telegram sent before Requests had pages of their own; the fragment never reaches the
    // Server, so only the app can send them on.
    const requestId = /^request-(\d+)$/u.exec(location.hash)?.[1];
    if (requestId !== undefined) {
      throw redirect({ to: "/requests/$requestId", params: { requestId }, replace: true });
    }
    // The default goes into the URL, replacing it, so it stays put once the page is open (approving
    // the last Pending Request leaves the page on Pending) and Back leaves the page.
    if (search.state === undefined) {
      const state = await defaultFilter(context, context.signedIn.user);
      throw redirect({ to: "/requests", search: { ...search, state }, replace: true });
    }
  },
  validateSearch: (
    search: Record<string, unknown>,
  ): { title?: string; state?: StateFilter | undefined } => ({
    ...(typeof search["title"] === "string" && search["title"] !== ""
      ? { title: search["title"] }
      : {}),
    // Set even when unknown: the router keeps the raw value under any key this leaves out.
    state: isStateFilter(search["state"]) ? search["state"] : undefined,
  }),
  loaderDeps: ({ search: { state } }) => ({ state }),
  loader: ({ context: { queryClient, transport }, deps: { state } }) =>
    queryClient.infiniteQuery(requestsQuery(state, transport)),
  component: Requests,
});

function Requests() {
  const { signedIn } = Route.useRouteContext();
  const { title, state = "all" } = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <RequestsPage
      user={signedIn.user}
      title={title}
      filter={state}
      choose={(chosen) => {
        void navigate({ search: (search) => ({ ...search, state: chosen }) });
      }}
    />
  );
}
