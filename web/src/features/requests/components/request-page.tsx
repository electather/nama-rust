import { Code } from "@connectrpc/connect";

import { RequestRow, ShowRequests } from "@/components/request-row";
import { Removed } from "@/features/requests/components/removed";
import { RequestLayout } from "@/features/requests/components/request-layout";
import { useRequest } from "@/features/requests/data/use-request";
import { isCode } from "@/shared/lib/connect-errors";

/** One Request, where a Notifier's link to it lands (`ServerUrl::request_link` on the Server). */
export function RequestPage({ id }: { id: bigint }) {
  const { data, error } = useRequest(id);
  const { request } = data;
  // Cancelling it refetches it, and the `not_found` that comes back leaves the old data in place.
  if (!request || isCode(error, Code.NotFound)) {
    return <Removed />;
  }
  const { movieOrSeason } = request;
  return (
    <RequestLayout>
      <ul className="mt-8 flex max-w-2xl flex-col">
        {movieOrSeason.case === "season" ? (
          <ShowRequests
            showTitle={movieOrSeason.value.showTitle}
            followed={[]}
            seasons={[request]}
          />
        ) : (
          <RequestRow request={request} />
        )}
      </ul>
    </RequestLayout>
  );
}
