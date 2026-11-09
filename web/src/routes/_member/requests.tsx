import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { FindATitle, type Searchable, TitlesFound } from "@/components/find-a-title";
import { Page } from "@/components/page";
import { RequestActions } from "@/components/request-actions";
import {
  RequestProgress,
  RequestStateLabel,
  type StateFilter,
  StateFilterMenu,
  filterName,
  isStateFilter,
  listedIn,
} from "@/components/request-state-filter";
import {
  type FollowedShow,
  type QualityChange,
  type Request,
  RequestService,
  type RequestedQuality,
} from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { dayOf, metadataLine, seasonName, yearOf } from "@/lib/browse";
import { may } from "@/lib/permissions";

export const Route = createFileRoute("/_member/requests")({
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
    queryClient.query({
      ...createQueryOptions(RequestService.method.listRequests, listedIn(state), { transport }),
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

/** The Requests and Followed Shows in the state the URL names, under the filter that chooses it. */
function RequestList() {
  const { state: filter = "all" } = Route.useSearch();
  const { data } = useSuspenseQuery(RequestService.method.listRequests, listedIn(filter));
  const navigate = Route.useNavigate();
  const choose = (state: StateFilter) =>
    void navigate({ search: (search) => ({ ...search, state }) });
  return (
    <>
      <div className="mt-8 -ms-4">
        <StateFilterMenu chosen={filter} choose={choose} />
      </div>
      {data.requests.length + data.followedShows.length === 0 ? (
        <p className="mt-4 text-muted-foreground">
          {filter === "all" ? "No Requests yet." : `No ${filterName(filter)} Requests.`}
        </p>
      ) : (
        <ul className="mt-4 flex max-w-2xl flex-col">
          {groupShows(data.requests, data.followedShows).map((entry) =>
            "seasons" in entry ? (
              <ShowRequests key={`show-${entry.showTmdbId}`} {...entry} />
            ) : (
              <RequestRow key={String(entry.id)} request={entry} />
            ),
          )}
        </ul>
      )}
    </>
  );
}

/** A Show's Followed Shows and Season Requests, listed together under it. */
interface ShowGroup {
  showTmdbId: bigint;
  showTitle: string;
  followed: FollowedShow[];
  seasons: Request[];
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

function ShowRequests({ showTitle, followed, seasons }: ShowGroup) {
  return (
    <li className="border-b pt-3 last:border-b-0">
      <p className="truncate font-medium">{showTitle}</p>
      <ul aria-label={showTitle} className="flex flex-col ps-4">
        {followed.map((followedShow) => (
          <RequestRow key={`followed-${followedShow.id}`} request={followedShow} />
        ))}
        {seasons.map((request) => (
          <RequestRow key={String(request.id)} request={request} />
        ))}
      </ul>
    </li>
  );
}

/** A Request, or a Followed Show, with its state and what the signed-in User may do to it. */
function RequestRow({ request }: { request: Request | FollowedShow }) {
  const { signedIn } = Route.useRouteContext();
  return (
    // Wraps so the downloads and the actions get lines of their own, the full width of the row.
    <li
      id={anchorOf(request)}
      className="flex min-h-16 flex-wrap items-center gap-x-3 border-b py-3 last:border-b-0"
    >
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{titleOf(request)}</p>
        <p className="text-sm text-muted-foreground">{aboutRequest(request, signedIn.user?.id)}</p>
        <Reason reason={reasonOf(request)} />
      </div>
      <RequestStateLabel state={request.state} />
      {request.$typeName === "nama.v1.Request" && <RequestProgress request={request} />}
      <RequestActions
        request={request}
        you={signedIn.user?.id}
        manage={may(signedIn.user, Permission.MANAGE_REQUESTS)}
      />
    </li>
  );
}

/** Where a Notifier's link to a Request lands (`ServerUrl::request_link` on the Server). */
function anchorOf(request: Request | FollowedShow) {
  return request.$typeName === "nama.v1.Request" ? `request-${request.id}` : undefined;
}

/**
 * A row's title: a Request's Movie's, or its Season's name, or "Followed Show", either of which
 * its Show's title is above.
 */
function titleOf(request: Request | FollowedShow) {
  if (request.$typeName === "nama.v1.FollowedShow") return "Followed Show";
  const { movie, season } = request;
  return season ? seasonName(season.seasonNumber) : movie?.title;
}

/**
 * What the row says under its title: the year a Request's title came out, its Quality, and who
 * asked.
 */
function aboutRequest(request: Request | FollowedShow, you: bigint | undefined) {
  if (request.$typeName === "nama.v1.FollowedShow") return requestedBy(request, you);
  return metadataLine(yearOf(releasedOf(request)), qualityOf(request), requestedBy(request, you));
}

/**
 * The Quality a Request is downloaded at: the one its service used once handed off, else the one
 * asked for, a profile no Quality offers being the service's own. Then who last changed it from
 * what, and when: "HD, changed from 4K by Sam on 9 Nov 2026".
 */
function qualityOf({ quality, qualityUsed, qualityChange }: Request) {
  const shown = qualityUsed ?? quality;
  if (shown === undefined) return shown;
  return qualityChange ? `${nameOf(shown)}, ${changeText(qualityChange)}` : nameOf(shown);
}

/** A Quality's name, or the service's own profile's when no Quality offers it. */
function nameOf({ id, name }: RequestedQuality) {
  return id === 0n ? `${name} (set in the service)` : name;
}

/** Who changed a Quality from what, and when, in words: "changed from 4K by Sam on 9 Nov 2026". */
function changeText({ previousName, changedBy = "a Request manager", changedAt }: QualityChange) {
  const from = previousName === undefined ? "" : ` from ${previousName}`;
  return `changed${from} by ${changedBy} on ${dayOf(changedAt)}`;
}

/** When a Request's Movie came out, or its Season first aired. */
function releasedOf({ season, movie }: Request) {
  return season?.airDate ?? movie?.releaseDate;
}

/** Why it was Declined, or why a Request Failed, in the decliner's or the Fulfiller's words. */
function reasonOf(request: Request | FollowedShow) {
  return request.declineReason ?? ("failureReason" in request ? request.failureReason : undefined);
}

function Reason({ reason }: { reason: string | undefined }) {
  return (
    reason !== undefined && (
      <p className="text-sm text-pretty wrap-break-word text-muted-foreground">Reason: {reason}</p>
    )
  );
}

// "en-GB" for "Ada, Grace and you", without the serial comma, as the rest of the app writes lists.
const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** Who asked, first asker first, with the signed-in User `you` as "you". */
function requestedBy({ requesters }: Pick<Request, "requesters">, you: bigint | undefined) {
  const who = requesters.map(({ userId, name }) => (userId === you ? "you" : name));
  return `Requested by ${names.format(who)}`;
}
