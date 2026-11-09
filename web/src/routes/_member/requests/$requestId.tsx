import { Code } from "@connectrpc/connect";
import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Page } from "@/components/page";
import { RequestRow, ShowRequests } from "@/components/request-row";
import { buttonVariants } from "@/components/ui/button";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { queryOrNotFound } from "@/lib/browse";
import { isCode } from "@/lib/connect-errors";

/** One Request, where a Notifier's link to it lands (`ServerUrl::request_link` on the Server). */
export const Route = createFileRoute("/_member/requests/$requestId")({
  loader: async ({ context, params }) => {
    if (!/^\d+$/u.test(params.requestId)) throw notFound();
    const input = { id: BigInt(params.requestId) };
    await queryOrNotFound(context, RequestService.method.getRequest, input);
    return input;
  },
  notFoundComponent: Removed,
  component: RequestPage,
});

function RequestPage() {
  const input = Route.useLoaderData();
  const { data, error } = useSuspenseQuery(RequestService.method.getRequest, input);
  const { request } = data;
  // Cancelling it refetches it, and the `not_found` that comes back leaves the old data in place.
  if (!request || isCode(error, Code.NotFound)) return <Removed />;
  return (
    <RequestLayout>
      <ul className="mt-8 flex max-w-2xl flex-col">
        {request.season ? (
          <ShowRequests showTitle={request.season.showTitle} followed={[]} seasons={[request]} />
        ) : (
          <RequestRow request={request} />
        )}
      </ul>
    </RequestLayout>
  );
}

function Removed() {
  return (
    <RequestLayout>
      <p className="mt-8 text-muted-foreground">
        This Request was removed, or isn’t one you can see.
      </p>
    </RequestLayout>
  );
}

function RequestLayout({ children }: { children: ReactNode }) {
  return (
    <Page heading="Request">
      {/* Pulled back by the button's own padding, so its words line up with the heading. */}
      <div className="mt-2 -ms-4">
        <Link to="/requests" className={buttonVariants({ variant: "ghost" })}>
          All Requests
        </Link>
      </div>
      {children}
    </Page>
  );
}
