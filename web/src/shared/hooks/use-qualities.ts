import { callUnaryMethod, createConnectQueryKey, useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

const { listMovieQualities, listShowQualities } = RequestService.method;

/** The Qualities of the Fulfiller of a Movie or a Show, as `kind` says, and the User's own default. */
export function useQualities(kind: "movie" | "show") {
  const transport = useTransport();
  const finite = { input: {}, transport, cardinality: "finite" } as const;
  // One query whichever the kind, so it's one hook call; each branch names its method, since
  // neither connect-query nor TanStack can type a call for either of two.
  return useSuspenseQuery({
    queryKey:
      kind === "movie"
        ? createConnectQueryKey({ schema: listMovieQualities, ...finite })
        : createConnectQueryKey({ schema: listShowQualities, ...finite }),
    queryFn: async () => {
      const listed = await (kind === "movie"
        ? callUnaryMethod(transport, listMovieQualities, {})
        : callUnaryMethod(transport, listShowQualities, {}));
      return listed;
    },
  }).data;
}
