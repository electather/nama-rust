import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { FindAMovie, MoviesFound } from "@/components/find-a-movie";
import { Page } from "@/components/page";
import { RequestActions } from "@/components/request-actions";
import { RequestStateLabel } from "@/components/request-state";
import { type Request, type RequestedSeason, RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { metadataLine, seasonName, yearOf } from "@/lib/browse";
import { may } from "@/lib/permissions";

export const Route = createFileRoute("/_member/requests")({
  validateSearch: (search: Record<string, unknown>): { title?: string } =>
    typeof search["title"] === "string" && search["title"] !== "" ? { title: search["title"] } : {},
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(RequestService.method.listRequests, {}, { transport }),
      staleTime: "static",
    }),
  component: Requests,
});

function Requests() {
  const { signedIn } = Route.useRouteContext();
  const { title } = Route.useSearch();
  return (
    <Page heading="Requests">
      {may(signedIn.user, Permission.REQUEST_MOVIE) ? (
        <FindOrList title={title} you={signedIn.user?.id} />
      ) : (
        <RequestList />
      )}
    </Page>
  );
}

/** The search, then what it found for `title`, or the Requests before anything is searched for. */
function FindOrList({ title, you }: { title: string | undefined; you: bigint | undefined }) {
  return (
    <>
      <FindAMovie title={title} />
      {title === undefined ? <RequestList /> : <MoviesFound title={title} you={you} />}
    </>
  );
}

function RequestList() {
  const { data } = useSuspenseQuery(RequestService.method.listRequests, {});
  return data.requests.length === 0 ? (
    <p className="mt-8 text-muted-foreground">No Requests yet.</p>
  ) : (
    <ul className="mt-8 flex max-w-2xl flex-col">
      {groupShows(data.requests).map((entry) =>
        "seasons" in entry ? (
          <ShowRequests key={`show-${entry.show.showTmdbId}`} {...entry} />
        ) : (
          <RequestRow key={String(entry.id)} request={entry} />
        ),
      )}
    </ul>
  );
}

/** A Show's Season Requests, listed together under it. */
interface ShowGroup {
  show: RequestedSeason;
  seasons: Request[];
}

/**
 * `requests` in their order, but with each Show's Season Requests together, in Season order,
 * where the Show's first one was.
 */
function groupShows(requests: Request[]) {
  const shows = new Map<bigint, ShowGroup>();
  const grouped = requests.flatMap((request): (Request | ShowGroup)[] => {
    const { season } = request;
    if (season === undefined) return [request];
    const group = shows.get(season.showTmdbId);
    if (group !== undefined) {
      group.seasons.push(request);
      return [];
    }
    const first = { show: season, seasons: [request] };
    shows.set(season.showTmdbId, first);
    return [first];
  });
  for (const { seasons } of shows.values()) {
    seasons.sort((a, b) => seasonNumber(a) - seasonNumber(b));
  }
  return grouped;
}

function seasonNumber(request: Request) {
  return request.season?.seasonNumber ?? 0;
}

function ShowRequests({ show, seasons }: ShowGroup) {
  return (
    <li className="border-b pt-3 last:border-b-0">
      <p className="truncate font-medium">{show.showTitle}</p>
      <ul aria-label={show.showTitle} className="flex flex-col ps-4">
        {seasons.map((request) => (
          <RequestRow key={String(request.id)} request={request} />
        ))}
      </ul>
    </li>
  );
}

function RequestRow({ request }: { request: Request }) {
  const { signedIn } = Route.useRouteContext();
  return (
    // Wraps so the actions get a line of their own, the full width of the row.
    <li className="flex min-h-16 flex-wrap items-center gap-x-3 border-b py-3 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{titleOf(request)}</p>
        <p className="text-sm text-muted-foreground">{aboutRequest(request, signedIn.user?.id)}</p>
        <Reason reason={request.declineReason ?? request.failureReason} />
      </div>
      <RequestStateLabel state={request.state} />
      <RequestActions
        request={request}
        you={signedIn.user?.id}
        manage={may(signedIn.user, Permission.MANAGE_REQUESTS)}
      />
    </li>
  );
}

/** A Request's title: its Movie's, or its Season's name, which its Show's title is above. */
function titleOf({ movie, season }: Request) {
  return season ? seasonName(season.seasonNumber) : movie?.title;
}

/** What the row says under a Request's title: the year it came out, and who asked for it. */
function aboutRequest(request: Request, you: bigint | undefined) {
  const released = request.season?.airDate ?? request.movie?.releaseDate;
  return metadataLine(yearOf(released), requestedBy(request, you));
}

/** Why a Request was Declined, or why it Failed, in the decliner's or the Fulfiller's words. */
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
function requestedBy({ requesters }: Request, you: bigint | undefined) {
  const who = requesters.map(({ userId, name }) => (userId === you ? "you" : name));
  return `Requested by ${names.format(who)}`;
}
