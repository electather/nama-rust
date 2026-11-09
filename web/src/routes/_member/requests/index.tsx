import { createInfiniteQueryOptions, useSuspenseInfiniteQuery } from "@connectrpc/connect-query";
import { createFileRoute, redirect } from "@tanstack/react-router";

import { FindATitle, type Searchable, TitlesFound } from "@/components/find-a-title";
import { Page } from "@/components/page";
import { RequestRow, type ShowGroup, ShowRequests } from "@/components/request-row";
import { ShowMore } from "@/components/show-more";
import {
  type StateFilter,
  StateFilterMenu,
  defaultFilter,
  filterName,
  isStateFilter,
  listedIn,
} from "@/components/request-state-filter";
import {
  type FollowedShow,
  type ListRequestsResponse,
  type Request,
  RequestService,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/lib/permissions";

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
    queryClient.infiniteQuery({
      ...createInfiniteQueryOptions(RequestService.method.listRequests, firstPageIn(state), {
        transport,
        ...paging,
      }),
      staleTime: "static",
    }),
  component: Requests,
});

function Requests() {
  const { signedIn } = Route.useRouteContext();
  const { title } = Route.useSearch();
  const kinds = {
    movies: may(signedIn.user, Permission.REQUEST_MOVIE),
    shows: may(signedIn.user, Permission.REQUEST_TV),
  };
  return (
    <Page heading="Requests">
      {kinds.movies || kinds.shows ? (
        <FindOrList title={title} kinds={kinds} you={signedIn.user?.id} />
      ) : (
        <RequestList />
      )}
    </Page>
  );
}

/** The search, then what it found for `title`, or the Requests before anything is searched for. */
function FindOrList({
  title,
  kinds,
  you,
}: {
  title: string | undefined;
  kinds: Searchable;
  you: bigint | undefined;
}) {
  return (
    <>
      <FindATitle title={title} kinds={kinds} />
      {title === undefined ? (
        <RequestList />
      ) : (
        <TitlesFound title={title} kinds={kinds} you={you} />
      )}
    </>
  );
}

/** `ListRequests`' first page of the Requests and Followed Shows in `filter`. */
function firstPageIn(filter: StateFilter | undefined) {
  return { ...listedIn(filter), pageSize: 50, pageToken: "" };
}

/** How each `ListRequests` page asks for the next, until the last says there is none. */
const paging = {
  pageParamKey: "pageToken",
  getNextPageParam: (page: ListRequestsResponse) => page.nextPageToken || undefined,
} as const;

/**
 * The Requests and Followed Shows in the state the URL names, a page at a time, under the filter
 * that chooses it.
 */
function RequestList() {
  const { state: filter = "all" } = Route.useSearch();
  const { data, hasNextPage, fetchNextPage } = useSuspenseInfiniteQuery(
    RequestService.method.listRequests,
    firstPageIn(filter),
    paging,
  );
  const requests = data.pages.flatMap((page) => page.requests);
  const followedShows = data.pages.flatMap((page) => page.followedShows);
  const navigate = Route.useNavigate();
  const choose = (state: StateFilter) =>
    void navigate({ search: (search) => ({ ...search, state }) });
  return (
    <>
      <div className="mt-8 -ms-4">
        <StateFilterMenu chosen={filter} choose={choose} />
      </div>
      {requests.length + followedShows.length === 0 ? (
        <p className="mt-4 text-muted-foreground">
          {filter === "all" ? "No Requests yet." : `No ${filterName(filter)} Requests.`}
        </p>
      ) : (
        <ul className="mt-4 flex max-w-2xl flex-col">
          {groupShows(requests, followedShows).map((entry) =>
            "seasons" in entry ? (
              <ShowRequests key={`show-${entry.showTmdbId}`} {...entry} />
            ) : (
              <RequestRow key={String(entry.id)} request={entry} />
            ),
          )}
        </ul>
      )}
      {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
    </>
  );
}

/**
 * `requests` and `followedShows`, newest first, but with each Show's together where the Show's
 * newest one was: its Followed Shows, then its Season Requests in Season order.
 */
function groupShows(requests: Request[], followedShows: FollowedShow[]) {
  const shows = new Map<bigint, ShowGroup>();
  const newestFirst = [...requests, ...followedShows];
  // Sorting is stable, so Requests opened in the same second keep the Server's order.
  newestFirst.sort((a, b) => Number(b.createdAt - a.createdAt));
  const grouped = newestFirst.flatMap((asked): (Request | FollowedShow | ShowGroup)[] => {
    const show = showOf(asked);
    if (show === undefined) return [asked];
    const known = shows.get(show.showTmdbId);
    const group = known ?? newGroup(show);
    addTo(group, asked);
    shows.set(show.showTmdbId, group);
    return known ? [] : [group];
  });
  for (const { seasons } of shows.values()) {
    seasons.sort((a, b) => seasonNumber(a) - seasonNumber(b));
  }
  return grouped;
}

type Show = Pick<ShowGroup, "showTmdbId" | "showTitle">;

/** The Show a Followed Show or a Season Request is of; a Movie's Request has none. */
function showOf(asked: Request | FollowedShow): Show | undefined {
  return asked.$typeName === "nama.v1.FollowedShow" ? asked : asked.season;
}

function newGroup({ showTmdbId, showTitle }: Show): ShowGroup {
  return { showTmdbId, showTitle, followed: [], seasons: [] };
}

function addTo(group: ShowGroup, asked: Request | FollowedShow) {
  if (asked.$typeName === "nama.v1.FollowedShow") group.followed.push(asked);
  else group.seasons.push(asked);
}

function seasonNumber(request: Request) {
  return request.season?.seasonNumber ?? 0;
}
