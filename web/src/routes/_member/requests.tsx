import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { Page } from "@/components/page";
import { RequestStateLabel } from "@/components/request-state";
import { type Request, RequestService } from "@/gen/nama/v1/request_pb";
import { metadataLine, yearOf } from "@/lib/browse";

export const Route = createFileRoute("/_member/requests")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(RequestService.method.listRequests, {}, { transport }),
      staleTime: "static",
    }),
  component: Requests,
});

function Requests() {
  const { data } = useSuspenseQuery(RequestService.method.listRequests, {});
  return (
    <Page heading="Requests">
      {data.requests.length === 0 ? (
        <p className="mt-8 text-muted-foreground">No Requests yet.</p>
      ) : (
        <ul className="mt-8 flex max-w-2xl flex-col">
          {data.requests.map((request) => (
            <RequestRow key={String(request.id)} request={request} />
          ))}
        </ul>
      )}
    </Page>
  );
}

function RequestRow({ request }: { request: Request }) {
  const { signedIn } = Route.useRouteContext();
  return (
    <li className="flex min-h-16 items-center gap-3 border-b py-3 last:border-b-0">
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{request.movie?.title}</p>
        <p className="text-sm text-muted-foreground">
          {metadataLine(
            yearOf(request.movie?.releaseDate),
            requestedBy(request, signedIn.user?.id),
          )}
        </p>
      </div>
      <RequestStateLabel state={request.state} />
    </li>
  );
}

// "en-GB" for "Ada, Grace and you", without the serial comma, as the rest of the app writes lists.
const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** Who asked, first asker first, with the signed-in User `you` as "you". */
function requestedBy({ requesters }: Request, you: bigint | undefined) {
  const who = requesters.map(({ userId, name }) => (userId === you ? "you" : name));
  return `Requested by ${names.format(who)}`;
}
