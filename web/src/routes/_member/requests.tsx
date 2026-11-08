import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { FindAMovie, MoviesFound } from "@/components/find-a-movie";
import { Page } from "@/components/page";
import { RequestActions } from "@/components/request-actions";
import { RequestStateLabel } from "@/components/request-state";
import { type Request, RequestService } from "@/gen/nama/v1/request_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { metadataLine, yearOf } from "@/lib/browse";
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
      {data.requests.map((request) => (
        <RequestRow key={String(request.id)} request={request} />
      ))}
    </ul>
  );
}

function RequestRow({ request }: { request: Request }) {
  const { signedIn } = Route.useRouteContext();
  return (
    // Wraps so the actions get a line of their own, the full width of the row.
    <li className="flex min-h-16 flex-wrap items-center gap-x-3 border-b py-3 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{request.movie?.title}</p>
        <p className="text-sm text-muted-foreground">{aboutRequest(request, signedIn.user?.id)}</p>
        <DeclineReason reason={request.declineReason} />
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

/** What the row says under a Request's title: the Movie's year, and who asked for it. */
function aboutRequest(request: Request, you: bigint | undefined) {
  return metadataLine(yearOf(request.movie?.releaseDate), requestedBy(request, you));
}

function DeclineReason({ reason }: { reason: string | undefined }) {
  return (
    reason !== undefined && (
      <p className="text-sm text-pretty text-muted-foreground">Reason: {reason}</p>
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
